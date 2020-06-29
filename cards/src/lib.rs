#![recursion_limit="512"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::time::Instant;
use std::path::Path;
// use std::collections::HashSet;

#[macro_use] extern crate lazy_static;

use futures::{
    Future,
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};

mod file_spec;
mod check;
mod save;
mod fetch;

// ============================================================================
// ============================================================================

pub struct Arg<'a, F> 
where 
    F: Future<Output=Result<String>>
{
    pub get_auth: fn() -> F, // https://stackoverflow.com/questions/58173711/how-can-i-store-an-async-function-in-a-struct-and-call-it-from-a-struct-instance
    pub ids: &'a ids::Ret, 
    pub out_dir: &'a Path, 
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub retry_count: usize,
}

macro_rules! push_fut_fetch {
    ($fut_queue: expr, $client: expr, $auth: expr, $arg: expr, $ids_non_existent: expr, $ids_non_existent_i: expr) => {
        let id = $ids_non_existent[$ids_non_existent_i];
        let auth = match &$auth {
            Some(auth) => auth.to_owned(),
            None => {
                let auth = ($arg.get_auth)().await?;
                $auth = Some(auth.to_owned());
                auth
            },
        };
        let arg = OpArg::Fetch (fetch::Arg {
            client: $client,
            auth,
            id: id,
            retry_count: $arg.retry_count,
        });
        let fut = op(arg);
        $fut_queue.push(fut);
        $ids_non_existent_i += 1;
    };
}

macro_rules! push_fut_save {
    ($fut_queue: expr, $json: expr, $id: expr, $out_dir: expr) => {
        let arg = OpArg::Save (save::Arg {
            id: $id,
            json: $json,
            out_dir: $out_dir
        });
        let fut = op(arg);
        $fut_queue.push(fut);
    };
}

macro_rules! push_fut_check {
    ($fut_queue: expr, $id: expr, $out_dir: expr) => {
        let arg = OpArg::Check (check::Arg {
            id: $id,
            out_dir: $out_dir
        });
        let fut = op(arg);
        $fut_queue.push(fut);
    };
}

pub struct CallbackArg {
    pub elapsed_qt: usize,
    pub remained_qt: usize,
    pub elapsed_millis: u128,
    pub remained_millis: u128,
    pub per_millis: u128,
}

// pub async fn non_existent_only<'a>(
//     ids: &'a ids::Ret, 
//     out_dir: &'a Path, 
//     thread_limit_file: usize, 
//     callback: Option<&dyn Fn(CallbackArg)>,
// ) -> Result<ids::Ret> {
//     let mut now = Instant::now();
//     let mut ids_non_existent = HashSet::new();
//
//     let mut fut_queue = FuturesUnordered::new();
//
//     let mut id_i = 0;
//     let ids_len = ids.len();
//     let ids = ids.iter().collect::<Vec<&u64>>();
//     while id_i < thread_limit_file && id_i < ids_len {
//         let id = *ids[id_i];
//         push_fut_check!(fut_queue, id, out_dir);
//         id_i += 1;
//     }
//     let mut elapsed_qt = 0;
//     let mut remained_qt = ids.len();
//     let mut last_callback = Instant::now();
//     info!("fut_queue.len(): {}", fut_queue.len());
//     loop {
//         select! {
//             ret = fut_queue.select_next_some() => {
//                 match ret {
//                     Err(err) => {
//                         return Err(err.context("cards::non_existent_only"));
//                     },
//                     Ok(ret) => {
//                         match ret {
//                             OpRet::Check(check::Ret{id}) => {
//                                 if let Some(callback) = callback {
//                                     elapsed_qt += 1;
//                                     if remained_qt > 0 {
//                                         remained_qt -= 1;
//                                     }
//                                     if Instant::now().duration_since(last_callback).as_millis() > 100 {
//                                         let elapsed_millis = Instant::now().duration_since(now).as_millis(); 
//                                         let per_millis = elapsed_millis / elapsed_qt as u128;
//                                         let remained_millis = per_millis * remained_qt as u128;
//                                         callback(CallbackArg {
//                                             elapsed_qt,
//                                             remained_qt,
//                                             elapsed_millis, 
//                                             remained_millis, 
//                                             per_millis,
//                                         });
//                                         last_callback = Instant::now();
//                                     }
//                                 }
//                                 if let Some(id) = id {
//                                     if ids_non_existent.len() == 0 {
//                                         now = Instant::now();
//                                     }
//                                     ids_non_existent.insert(id);
//                                 }
//                                 if id_i < ids_len {
//                                     let client = reqwest::Client::new();
//                                     let id = *ids[id_i];
//                                     push_fut_check!(fut_queue, id, out_dir);
//                                     id_i += 1;
//                                 }
//                             },
//                             _ => unreachable!(),
//                         }
//                     },
//                 }
//             },
//             complete => {
//                 break;
//             },
//         }
//     }
//     info!("{}, cards::non_existent_only: ret.len(): {}", 
//         arrange_millis::get(Instant::now().duration_since(now).as_millis()), 
//         ids_non_existent.len(),
//     );
//     
//     Ok(ids_non_existent)
// }

pub async fn fetch_and_save<'a, F: Future<Output=Result<String>>>(
    arg: &Arg<'a, F>, 
    callback: Option<&dyn Fn(CallbackArg)>,
) -> Result<()> {
    let now = Instant::now();
    let mut fut_queue = FuturesUnordered::new();

    let mut ids_non_existent: Vec<u64> = Vec::new();
    let mut ids_non_existent_i = 0;

    let mut auth: Option<String> = None;
    let mut id_i = 0;
    let ids_len = arg.ids.len();
    let ids = arg.ids.iter().collect::<Vec<&u64>>();
    while id_i < arg.thread_limit_file && id_i < ids_len {
        let id = *ids[id_i];
        push_fut_check!(fut_queue, id, arg.out_dir);
        id_i += 1;
    }
    let mut used_network_threads = 0;

    let mut elapsed_qt = 0;
    let mut remained_qt = 0;
    let mut last_callback = Instant::now();
    let mut start_fetch: Option<Instant> = None; 
    loop {
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(err) => {
                        return Err(err.context("cards::fetch_and_save"));
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::Check(check::Ret{id}) => {
                                if let Some(id) = id {
                                    if ids_non_existent.len() == 0 {
                                        start_fetch = Some(Instant::now());
                                    }
                                    ids_non_existent.push(id);
                                    if let Some(callback) = callback {
                                        remained_qt += 1;
                                        if let Some(start_fetch) = start_fetch {
                                            if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > 100 {
                                                let elapsed_millis = Instant::now().duration_since(start_fetch).as_millis(); 
                                                let per_millis = elapsed_millis / elapsed_qt as u128;
                                                let remained_millis = per_millis * remained_qt as u128;
                                                callback(CallbackArg {
                                                    elapsed_qt,
                                                    remained_qt,
                                                    elapsed_millis, 
                                                    remained_millis, 
                                                    per_millis,
                                                });
                                                last_callback = Instant::now();
                                            }
                                        }
                                    }
                                    if used_network_threads < arg.thread_limit_network {
                                        let client = reqwest::Client::new();
                                        push_fut_fetch!(fut_queue, client, auth, arg, ids_non_existent, ids_non_existent_i);
                                        used_network_threads += 1;
                                    }
                                }
                                if id_i < ids_len {
                                    let id = *ids[id_i];
                                    push_fut_check!(fut_queue, id, arg.out_dir);
                                    id_i += 1;
                                }
                            },
                            OpRet::Save(_) => {},
                            OpRet::Fetch(ret) => {
                                if let Some(callback) = callback {
                                    elapsed_qt += 1;
                                    if remained_qt > 0 {
                                        remained_qt -= 1;
                                    }
                                    if let Some(start_fetch) = start_fetch {
                                        if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > 100 {
                                            let elapsed_millis = Instant::now().duration_since(start_fetch).as_millis(); 
                                            let per_millis = elapsed_millis / elapsed_qt as u128;
                                            let remained_millis = per_millis * remained_qt as u128;
                                            callback(CallbackArg {
                                                elapsed_qt,
                                                remained_qt,
                                                elapsed_millis, 
                                                remained_millis, 
                                                per_millis,
                                            });
                                            last_callback = Instant::now();
                                        }
                                    }
                                }
                                if let Some(json) = ret.json {
                                    push_fut_save!(fut_queue, json, ret.id, arg.out_dir);
                                }
                                if ids_non_existent_i < ids_non_existent.len() {
                                    let client = ret.client;
                                    push_fut_fetch!(fut_queue, client, auth, arg, ids_non_existent, ids_non_existent_i);
                                } else {
                                    used_network_threads -= 1;
                                }
                            },
                        }
                    },
                }
            },
            complete => {
                break;
            },
        }
    }
    info!("{}, cards::fetch_and_save", 
        arrange_millis::get(Instant::now().duration_since(now).as_millis()), 
    );
    
    Ok(())
}

enum OpArg<'a> {
    Fetch(fetch::Arg),
    Save(save::Arg<'a>),
    Check(check::Arg<'a>),
}

enum OpRet {
    Fetch(fetch::Ret),
    Save(save::Ret),
    Check(check::Ret),
}

async fn op<'a>(arg: OpArg<'a>) -> Result<OpRet> {
    match arg {
        OpArg::Fetch(arg) => {
            let ret = fetch::run(arg).await?;
            Ok(OpRet::Fetch(ret))
        },
        OpArg::Save(arg) => {
            let ret = save::run(arg).await?;
            Ok(OpRet::Save(ret))
        },
        OpArg::Check(arg) => {
            let ret = check::run(arg).await?;
            Ok(OpRet::Check(ret))
        },
    }
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests;

