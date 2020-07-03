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

mod card;
mod file_spec;
mod check;
mod save;
mod fetch;

pub use card::{Card, Record};

// ============================================================================
// ============================================================================

// type Items = collect::Autocatalog;
pub struct Arg<'a> {
    pub items: &'a collect::Autocatalog, 
    pub out_dir: &'a Path, 
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub retry_count: usize,
}

macro_rules! push_fut_fetch {
    ($fut_queue: expr, $client: expr, $auth: expr, $arg: expr, $items_non_existent: expr, $items_non_existent_i: expr) => {
        let item = $items_non_existent[$items_non_existent_i];
        let arg = OpArg::Fetch (fetch::Arg {
            client: $client,
            auth: $auth.key().await?,
            item: item,
            retry_count: $arg.retry_count,
        });
        let fut = op(arg);
        $fut_queue.push(fut);
        $items_non_existent_i += 1;
    };
}

macro_rules! push_fut_save {
    ($fut_queue: expr, $card: expr, $item: expr, $out_dir: expr) => {
        let arg = OpArg::Save (save::Arg {
            item: $item,
            card: $card,
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
    auth: &mut auth::Lazy,
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let mut items_non_existent: Vec<String> = Vec::new();
    let mut items_non_existent_i: usize = 0;

    let mut items_i = 0;
    let items_len = arg.items.0.len();
    let items = arg.items.0.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

    let mut fut_queue = FuturesUnordered::new();
    while items_i < arg.thread_limit_file && items_i < items_len {
        let item = items[items_i].to_owned();
        push_fut_check!(fut_queue, item, arg.out_dir);
        items_i += 1;
    }
    let mut used_network_threads = 0;

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
                        return Err(err.context("cards::fetch_and_save"));
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::Save(_) => {},
                            OpRet::Check(check::Ret{id}) => {
                                if let Some(id) = id {
                                    if items_non_existent.len() == 0 {
                                        start = Some(Instant::now());
                                    }
                                    items_non_existent.push(id);
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
                                        push_fut_fetch!(fut_queue, client, auth, arg, items_non_existent, items_non_existent_i);
                                        used_network_threads += 1;
                                    }
                                }
                                if items_i < items_len {
                                    let item = items[items_i].to_owned();
                                    push_fut_check!(fut_queue, item, arg.out_dir);
                                    items_i += 1;
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
                                push_fut_save!(fut_queue, ret.card, ret.item, arg.out_dir);
                                if items_non_existent_i < items_non_existent.len() {
                                    let client = ret.client;
                                    push_fut_fetch!(fut_queue, client, auth, arg, items_non_existent, items_non_existent_i);
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
            callback!(callback, start, elapsed_qt, remained_qt);
        }
    }
    
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
mod tests {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    use std::collections::HashSet;

    #[tokio::test]
    async fn test_file_spec() -> Result<()> {
        init();

        let out_dir = &Path::new("out_test");

        let id = std::u64::MAX - 1;
        let fspec = file_spec::get(out_dir, id);
        assert_eq!(fspec.to_string_lossy(), "out_test/ff/ff/ff/ff/ff/ff/ff/fe.json");

        let out_dir = &Path::new("out");
        let id = std::u64::MAX;
        let fspec = file_spec::get(out_dir, id);
        assert_eq!(fspec.to_string_lossy(), "out/ff/ff/ff/ff/ff/ff/ff/ff.json");

        Ok(())
    }

    #[tokio::test]
    async fn test_check() -> Result<()> {
        init();

        let out_dir = &Path::new("out_test");
        let id = 42;

        let ret = check::run(check::Arg { out_dir, id }).await?;
        assert_eq!(ret, check::Ret{id: Some(id)});

        Ok(())
    }

    use term::Term;

    #[tokio::test]
    async fn test_fetch_and_save() -> Result<()> {
        init();

        let mut ids: ids::Ret = HashSet::new();
        let ids_vec: Vec<u64> = vec![
        1767797249
      // 1981851621,
      // 1981867820,
      // 1981886803,
      // 1981901279,
      // 1981920273,
      // 1981924600
        ];
        for id in ids_vec {
            ids.insert(id);
        }
        let out_dir = &Path::new("out_test");
        let arg = Arg {
            ids: &ids,
            out_dir,
            thread_limit_network: 1,
            thread_limit_file: 12,
            retry_count: 3,
        };
        let mut auth = auth::Lazy::new(Some(auth::Arg::new()));

        let mut term = Term::init(term::Arg::new().header("Получение объявлений . . ."))?;
        let start = Instant::now();
        let ret = fetch_and_save(&mut auth, arg, Some(|arg: CallbackArg| -> Result<()> {
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
        println!("{}, Объявления получены: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.received_qt);

        Ok(())
    }
}



