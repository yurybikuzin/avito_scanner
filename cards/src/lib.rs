#![recursion_limit="512"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::time::Instant;
use std::path::Path;

#[macro_use] extern crate lazy_static;

use futures::{
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

pub struct Arg<'a> {
    pub ids: &'a ids::Ret, 
    pub out_dir: &'a Path, 
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub retry_count: usize,
}

macro_rules! push_fut_fetch {
    ($fut_queue: expr, $client: expr, $auth: expr, $arg: expr, $ids_non_existent: expr, $ids_non_existent_i: expr) => {
        let id = $ids_non_existent[$ids_non_existent_i];
        let arg = OpArg::Fetch (fetch::Arg {
            client: $client,
            auth: $auth.key().await?,
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

pub struct Ret {
    pub received_qt: usize,
}

pub async fn fetch_and_save<'a, Cb>(
    auth: &mut auth::Lazy,
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let now = Instant::now();

    let mut ids_non_existent: Vec<u64> = Vec::new();
    let mut ids_non_existent_i = 0;

    let mut id_i = 0;
    let ids_len = arg.ids.len();
    let ids = arg.ids.iter().collect::<Vec<&u64>>();

    let mut fut_queue = FuturesUnordered::new();
    while id_i < arg.thread_limit_file && id_i < ids_len {
        let id = *ids[id_i];
        push_fut_check!(fut_queue, id, arg.out_dir);
        id_i += 1;
    }
    let mut used_network_threads = 0;

    let mut received_qt = 0;
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
                                    callback = if let Some(mut callback) = callback {
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
                                                })?;
                                                last_callback = Instant::now();
                                            }
                                        }
                                        Some(callback)
                                    } else {
                                        None
                                    };
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
                                callback = if let Some(mut callback) = callback {
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
                                            })?;
                                            last_callback = Instant::now();
                                        }
                                    }
                                    Some(callback)
                                } else {
                                    None
                                };
                                if let Some(json) = ret.json {
                                    received_qt += 1;
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
    
    Ok(Ret{received_qt})
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

