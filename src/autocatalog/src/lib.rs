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


macro_rules! fut_check {
    ($fut_queue: expr, $fut_count: ident, $fut_count_max: expr, $fut_fill_from: expr, $out_dir: expr) => {
        while $fut_count < $fut_count_max {
            match $fut_fill_from.pop_front() {
                None => break,
                Some(item) => {
                    let arg = op::Arg::Check (check::Arg {
                        item,
                        out_dir: $out_dir
                    });
                    $fut_queue.push(op::run(arg));
                    $fut_count += 1;
                },
            }
        }
    };
}

macro_rules! fut_fetch {
    ($fut_queue: expr, $fut_count: ident, $fut_count_max: expr, $fut_fill_from: expr, $client: block) => {
        while $fut_count < $fut_count_max {
            match $fut_fill_from.pop_front() {
                None => break,
                Some(item) => {
                    let client = $client;
                    fut_fetch!($fut_queue, $fut_count, item, client);
                },
            }
        }
    };
    ($fut_queue: expr, $fut_count: ident, $fut_count_max: expr, $fut_fill_from: expr, $client: expr) => {
        if $fut_count < $fut_count_max {
            if let Some(item) = $fut_fill_from.pop_front() {
                fut_fetch!($fut_queue, $fut_count, item, $client);
            }
        }
    };
    ($fut_queue: expr, $fut_count: ident, $item: expr, $client: expr) => {
        let arg = op::Arg::Fetch (fetch::Arg { item: $item, client: $client });
        $fut_queue.push(op::run(arg));
        $fut_count += 1;
    }
}

macro_rules! fut_save {
    ($fut_queue: expr, $fetched: expr, $item: expr, $out_dir: expr) => {
        let arg = op::Arg::Save (save::Arg {
            item: $item,
            fetched: $fetched,
            out_dir: $out_dir
        });
        $fut_queue.push(op::run(arg));
    };
}


macro_rules! callback {
    ($callback: ident, $start: expr, $last_callback: ident, $elapsed_qt: ident, $remained_qt: ident, $block: block) => {
        $callback = if let Some(mut callback) = $callback {

            $block;

            if let Some(start) = $start {
                if $elapsed_qt > 0 && Instant::now().duration_since($last_callback).as_millis() > CALLBACK_THROTTLE {
                    callback!(callback, start, $elapsed_qt, $remained_qt);
                    $last_callback = Instant::now();
                }
            }
            Some(callback)
        } else {
            None
        };

    };
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

pub struct Arg<'a> {
    pub items_to_check: &'a HashSet<String>, 
    pub out_dir: &'a Path, 
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub client_provider: client::Provider,
}

pub struct Ret {
    pub received_qt: usize,
}

use std::path::PathBuf;
use tokio::fs;
use tokio::prelude::*;
pub async fn get(out_dir: &Path, autocatalog_url: &str) -> Result<Record> {
    let s = format!("{}{}.json", out_dir.to_string_lossy(), autocatalog_url);
    let file_path = PathBuf::from(s);
    let mut file = fs::File::open(&file_path).await.context(format!("{:?}", file_path))?;
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).await?;
    let s = std::str::from_utf8(&buffer)?;
    let record = serde_json::from_str(s)?;
    Ok(record)
}

use std::collections::VecDeque;
const CALLBACK_THROTTLE: u128 = 100;
pub async fn fetch_and_save<'a, Cb>(
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let mut items_to_fetch: VecDeque<String> = VecDeque::new();

    let mut items_to_check: VecDeque<String> = VecDeque::with_capacity(arg.items_to_check.len()); 
    for item in arg.items_to_check.iter() {
        items_to_check.push_back(item.to_owned())
    }

    let mut items_to_fetch_uniq: HashSet<String> = HashSet::new();

    let mut fut_queue = FuturesUnordered::new();
    let mut fut_check_count = 0usize;
    let mut fut_fetch_count = 0usize;

    fut_check!(fut_queue, fut_check_count, arg.thread_limit_file, items_to_check, arg.out_dir);

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
                            op::Ret::Save(_) => {},
                            // Пришел результат проверки сущестсования ранее скачанного
                            // автокаталога
                            // Если автокаталог еще не скачан, то получаем Some(item_to_fetch)
                            op::Ret::Check(check::Ret{item_to_fetch}) => {
                                fut_check_count -= 1;
                                if let Some(item_to_fetch) = item_to_fetch {
                                    if items_to_fetch.len() == 0 {
                                        start = Some(Instant::now());
                                    }
                                    if !items_to_fetch_uniq.contains(&item_to_fetch) {
                                        items_to_fetch.push_back(item_to_fetch.to_owned());
                                        items_to_fetch_uniq.insert(item_to_fetch.to_owned());
                                    }

                                    callback!(callback, start, last_callback, elapsed_qt, remained_qt, {
                                        remained_qt += 1;
                                    });

                                    fut_fetch!(
                                        fut_queue, 
                                        fut_fetch_count, 
                                        arg.thread_limit_network, 
                                        items_to_fetch, 
                                        { arg.client_provider.build().await? } 
                                    );
                                }
                                fut_check!(fut_queue, fut_check_count, arg.thread_limit_file, items_to_check, arg.out_dir);
                            },
                            op::Ret::Fetch(ret) => {
                                fut_fetch_count -= 1;
                                fut_fetch!(fut_queue, fut_fetch_count, arg.thread_limit_network, items_to_fetch, ret.client );
                                // trace!("fetch: {:?}", ret.fetched);
                                callback!(callback, start, last_callback, elapsed_qt, remained_qt, {
                                    elapsed_qt += 1;
                                    if remained_qt > 0 {
                                        remained_qt -= 1;
                                    }
                                });

                                fut_save!(fut_queue, ret.fetched, ret.item, arg.out_dir);
                                received_qt += 1;
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

mod op {
    use super::*;

    type Item = str;
    pub enum Arg<'a, I: AsRef<Item>, P: AsRef<Path>> {
        Fetch(fetch::Arg<I>),
        Save(save::Arg<I, P>),
        Check(check::Arg<'a>),
    }

    pub enum Ret<I: AsRef<Item>> {
        Fetch(fetch::Ret<I>),
        Save(save::Ret),
        Check(check::Ret),
    }

    pub async fn run<'a, I: AsRef<Item>, P: AsRef<Path>>(arg: Arg<'a, I, P>) -> Result<Ret<I>> {
        match arg {
            Arg::Fetch(arg) => {
                let ret = fetch::run(arg).await?;
                Ok(Ret::Fetch(ret))
            },
            Arg::Save(arg) => {
                let ret = save::run(arg).await?;
                Ok(Ret::Save(ret))
            },
            Arg::Check(arg) => {
                let ret = check::run(arg).await?;
                Ok(Ret::Check(ret))
            },
        }
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
        INIT.call_once(|| pretty_env_logger::init());
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
        let settings = rmq::Settings::new(std::path::Path::new("../../cnf/rmq/bikuzin18.toml"))?;
        let arg = Arg {
            items_to_check: &items,
            out_dir,
            thread_limit_network: 50,
            thread_limit_file: 2,
            client_provider: client::Provider::new(client::Kind::ViaProxy(rmq::get_pool(settings)?, "autocatalog".to_owned())),
            // retry_count: 3,
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



