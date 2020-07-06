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

mod fetched;
mod file_spec;
mod check;
mod save;
mod fetch;

pub use fetched::{Fetched, Record};
use std::collections::HashSet;

// ============================================================================
// ============================================================================


// type Items = collect::Autocatalog;
pub struct Arg<'a> {
    // pub items_to_check: &'a collect::Autocatalog, 
    pub items_to_check: &'a HashSet<String>, 
    pub out_dir: &'a Path, 
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub retry_count: usize,
}

macro_rules! push_fut_fetch {
    ($fut_queue: expr, $client: expr, $arg: expr, $items_to_fetch: expr, $items_to_fetch_i: expr) => {
        let item = $items_to_fetch[$items_to_fetch_i].to_owned();
        let arg = OpArg::Fetch (fetch::Arg {
            client: $client,
            item: item,
            retry_count: $arg.retry_count,
        });
        let fut = op(arg);
        $fut_queue.push(fut);
        $items_to_fetch_i += 1;
    };
}

macro_rules! push_fut_save {
    ($fut_queue: expr, $fetched: expr, $item: expr, $out_dir: expr) => {
        let arg = OpArg::Save (save::Arg {
            item: $item,
            fetched: $fetched,
            out_dir: $out_dir
        });
        let fut = op(arg);
        $fut_queue.push(fut);
    };
}

macro_rules! push_fut_check {
    ($fut_queue: expr, $id: expr, $out_dir: expr) => {
        let arg = OpArg::Check (check::Arg {
            item: $id,
            out_dir: $out_dir
        });
        let fut = op(arg);
        $fut_queue.push(fut);
    };
}

macro_rules! callback {
    ($callback: expr, $start: expr, $elapsed_qt: expr, $remained_qt: expr) => {
        let elapsed_millis = Instant::now().duration_since($start).as_millis(); 
        let per_millis = elapsed_millis / $elapsed_qt as u128;
        let remained_millis = per_millis * $remained_qt as u128;
        $callback(CallbackArg {
            elapsed_qt: $elapsed_qt,
            remained_qt: $remained_qt,
            elapsed_millis, 
            remained_millis, 
            per_millis,
        })?;
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

const CALLBACK_THROTTLE: u128 = 100;
pub async fn fetch_and_save<'a, Cb>(
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let mut items_to_fetch: Vec<String> = Vec::new();
    let mut items_to_fetch_i: usize = 0;

    let mut items_to_check_i = 0;
    let items_to_check = arg.items_to_check.iter().cloned().collect::<Vec<String>>();

    let mut items_to_fetch_uniq: HashSet<String> = HashSet::new();

    let mut fut_queue = FuturesUnordered::new();
    while items_to_check_i < arg.thread_limit_file && items_to_check_i < items_to_check.len() {
        let item_to_check = items_to_check[items_to_check_i].to_owned();
        push_fut_check!(fut_queue, item_to_check, arg.out_dir);
        items_to_check_i += 1;
    }
    let mut used_network_threads: usize = 0;

    let mut received_qt = 0;
    let mut elapsed_qt = 0;
    let mut remained_qt = 0;
    let mut last_callback = Instant::now();
    let mut start: Option<Instant> = None; 
    loop {
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(err) => {
                        return Err(err.context("autocatalog::fetch_and_save"));
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::Save(_) => {},
                            OpRet::Check(check::Ret{item_to_fetch}) => {
                                if let Some(item_to_fetch) = item_to_fetch {
                                    if items_to_fetch.len() == 0 {
                                        start = Some(Instant::now());
                                    }
                                    if !items_to_fetch_uniq.contains(&item_to_fetch) {
                                        items_to_fetch.push(item_to_fetch.to_owned());
                                        items_to_fetch_uniq.insert(item_to_fetch.to_owned());
                                    }
                                    callback = if let Some(mut callback) = callback {
                                        remained_qt += 1;
                                        if let Some(start) = start {
                                            if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                                callback!(callback, start, elapsed_qt, remained_qt);
                                                last_callback = Instant::now();
                                            }
                                        }
                                        Some(callback)
                                    } else {
                                        None
                                    };
                                    if used_network_threads < arg.thread_limit_network {
                                        let client = reqwest::Client::new();
                                        push_fut_fetch!(fut_queue, client, arg, items_to_fetch, items_to_fetch_i);
                                        used_network_threads += 1;
                                    }
                                }
                                if items_to_check_i < items_to_check.len() {
                                    let item_to_check = items_to_check[items_to_check_i].to_owned();
                                    push_fut_check!(fut_queue, item_to_check, arg.out_dir);
                                    items_to_fetch_i += 1;
                                }
                            },
                            OpRet::Fetch(ret) => {
                                callback = if let Some(mut callback) = callback {
                                    elapsed_qt += 1;
                                    if remained_qt > 0 {
                                        remained_qt -= 1;
                                    }
                                    if let Some(start) = start {
                                        if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                            callback!(callback, start, elapsed_qt, remained_qt);
                                            last_callback = Instant::now();
                                        }
                                    }
                                    Some(callback)
                                } else {
                                    None
                                };
                                received_qt += 1;
                                push_fut_save!(fut_queue, ret.fetched, ret.item, arg.out_dir);
                                if items_to_fetch_i < items_to_fetch.len() {
                                    let client = ret.client;
                                    push_fut_fetch!(fut_queue, client, arg, items_to_fetch, items_to_fetch_i);
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
    if let Some(mut callback) = callback {
        if let Some(start) = start {
            if elapsed_qt > 0 {
                callback!(callback, start, elapsed_qt, remained_qt);
            }
        }
    }
    info!("items_to_fetch: {}", items_to_fetch.len());
    info!("items_to_fetch_uniq: {}", items_to_fetch_uniq.len());
    
    Ok(Ret{received_qt})
}

type Item = str;
enum OpArg<'a, I: AsRef<Item>, P: AsRef<Path>> {
    Fetch(fetch::Arg<I>),
    Save(save::Arg<I, P>),
    Check(check::Arg<'a>),
}

enum OpRet<I: AsRef<Item>> {
    Fetch(fetch::Ret<I>),
    Save(save::Ret),
    Check(check::Ret),
}

async fn op<'a, I: AsRef<Item>, P: AsRef<Path>>(arg: OpArg<'a, I, P>) -> Result<OpRet<I>> {
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
mod tests {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    use term::Term;
    use json::{Json};
    // use itertools::Itertools;
    use std::collections::HashSet;
    use std::iter::FromIterator;

    #[tokio::test]
    async fn test_fetch_and_save() -> Result<()> {
        init();

        let items = Json::from_file("test_data/autocatalog_urls.json").await?;

        let items = 
            items.iter_vec()?
            .map(|val| val.as_string().unwrap())
            .collect::<Vec<String>>()
        ;
        let items: HashSet<String> = HashSet::from_iter(items.iter().cloned());
        let out_dir = Path::new("out_test");
        let arg = Arg {
            items_to_check: &items,
            out_dir,
            thread_limit_network: 1,
            thread_limit_file: 2,
            retry_count: 3,
        };

        let mut term = Term::init(term::Arg::new().header("Получение автокаталога . . ."))?;
        let start = Instant::now();
        let ret = fetch_and_save(arg, Some(|arg: CallbackArg| -> Result<()> {
            term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
                arrange_millis::get(arg.elapsed_millis), 
                arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                arrange_millis::get(arg.remained_millis), 
                arrange_millis::get(arg.per_millis), 
                arg.elapsed_qt,
                arg.elapsed_qt + arg.remained_qt,
                arg.remained_qt,
            ))
        })).await?;
        println!("{}, Автокаталог получен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.received_qt);

        Ok(())
    }
}



