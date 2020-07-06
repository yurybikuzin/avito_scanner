#![recursion_limit="512"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

// use std::fmt;
// use serde_json::{
//     // Value, 
//     Map};
use tokio::fs::File;
use tokio::prelude::*;
// use std::convert::TryFrom;

// ============================================================================
// ============================================================================

// pub async fn fetch_list2() -> Result<Vec<String>> {
//     let body = reqwest::get("https://getfreeproxylists.blogspot.com/")
//         .await?
//         .text()
//         .await?
//     ;
//     // let mut file = File::create("out_test/proxies.txt").await?;
//     // file.write_all(body.as_bytes()).await?;
//     // let lines = body.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
//     // Ok(lines)
//     //
//     todo!();
// }
//
// ============================================================================

pub async fn fetch_list() -> Result<Vec<String>> {
    let body = reqwest::get("http://rootjazz.com/proxies/proxies.txt")
        .await?
        .text()
        .await?
    ;
    let mut file = File::create("out_test/proxies.txt").await?;
    file.write_all(body.as_bytes()).await?;
    let lines = body.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
    Ok(lines)
}

// ============================================================================

pub async fn read_list() -> Result<Vec<String>> {
    let mut file = File::open("out_test/proxies.txt").await?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).await?;
    let lines = contents.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
    Ok(lines)
}

// ============================================================================

use json::{Json, By};
pub async fn get_ip(client: reqwest::Client) -> Result<String> {
    let url = "https://bikuzin.baza-winner.ru/echo";
    let text = client.get(url)
        .send()
        .await?
        .text()
        .await?
    ;
    let json = Json::from_str(text, url)?;
    let ip = json.get([By::key("headers"), By::key("x-real-ip")])?.as_string()?;
    Ok(ip)
}

use std::time::{Instant, Duration};

macro_rules! push_fut_check {
    ($fut_queue: expr, $ip: expr, $items_to_check: expr, $items_to_check_i: expr, $used_network_threads: expr) => {
        let item = $items_to_check[$items_to_check_i].to_owned();
        // let url = format!("http://{}", item);
        let url = format!("http://{}", item);
        match reqwest::Proxy::all(&url) {
            Err(_err) => {
                // error!("{}: {}", url, err);
            },
            Ok(url) => {
                let client = reqwest::Client::builder()
                    .proxy(url)
                    .timeout(Duration::from_secs(10))
                    .build()?
                ;
                let arg = OpArg::Check (check::Arg {
                    ip: $ip.to_owned(),
                    client,
                    item,
                });
                let fut = op(arg);
                $fut_queue.push(fut);
                $used_network_threads += 1;
            },
        }
        $items_to_check_i += 1;
    };
}

macro_rules! callback {
    ($callback: expr, $start: expr, $elapsed_qt: expr, $remained_qt: expr, $ret_main_len: expr) => {
        let elapsed_millis = Instant::now().duration_since($start).as_millis(); 
        let per_millis = elapsed_millis / $elapsed_qt as u128;
        let remained_millis = per_millis * $remained_qt as u128;
        $callback(CallbackArg {
            ret_main_len: $ret_main_len,
            elapsed_qt: $elapsed_qt,
            remained_qt: $remained_qt,
            elapsed_millis, 
            remained_millis, 
            per_millis,
        })?;
    };
}

pub enum OpArg {
    Check(check::Arg)
}

pub enum OpRet {
    Check(check::Ret),
}

pub async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::Check(arg) => {
            let ret = check::run(arg).await?;
            Ok(OpRet::Check(ret))
        },
    }
}

use itertools::Itertools;
use futures::{
    // future,
    select,
    stream::{
        StreamExt,
        FuturesUnordered,
    },
};

pub struct CallbackArg {
    pub ret_main_len: usize,
    pub elapsed_qt: usize,
    pub remained_qt: usize,
    pub elapsed_millis: u128,
    pub remained_millis: u128,
    pub per_millis: u128,
}

pub const CALLBACK_THROTTLE: u128 = 100;

pub struct Arg {
    pub list: Vec<String>,
    pub thread_limit_network: usize,
}
pub type Ret = Vec<String>;
pub async fn get_anon<Cb>(
    arg: Arg, 
    mut callback: Option<Cb>
) -> Result<Ret> 
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let client = reqwest::Client::new();
    let ip = get_ip(client).await?;
    // info!("ip: {}", ip);
    let items_to_check = &arg.list;
    let mut items_to_check_i = 0;
    let mut used_network_threads: usize = 0;

    let mut fut_queue = FuturesUnordered::new();
    while items_to_check_i < arg.thread_limit_network && items_to_check_i < items_to_check.len() {
        push_fut_check!(fut_queue, ip, items_to_check, items_to_check_i, used_network_threads);
    }

    let mut ret_main: Vec<String> = Vec::new();
    let mut last_callback = Instant::now();
    let start = Instant::now();
    loop {
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(err) => {
                        return Err(err.context("get_anon"));
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::Check(check::Ret{item}) => {
                                used_network_threads -= 1;
                                if let Some(item) = item {
                                    ret_main.push(item);
                                }
                                callback = if let Some(mut callback) = callback {
                                    let remained_qt = items_to_check.len() - items_to_check_i;
                                    let elapsed_qt = items_to_check_i;
                                    if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                        callback!(callback, start, elapsed_qt, remained_qt, ret_main.len());
                                        last_callback = Instant::now();
                                    }
                                    Some(callback)
                                } else {
                                    None
                                };
                                while items_to_check_i < arg.thread_limit_network && items_to_check_i < items_to_check.len() {
                                    push_fut_check!(fut_queue, ip, items_to_check, items_to_check_i, used_network_threads);
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
        let remained_qt = items_to_check.len() - items_to_check_i;
        let elapsed_qt = items_to_check_i;
        if elapsed_qt > 0 {
            callback!(callback, start, elapsed_qt, remained_qt, ret_main.len());
        }
    }
    let mut file = File::create("out_test/checked.txt").await?;
    let body = ret_main.iter().cloned().intersperse("\n".to_owned()).collect::<String>();
    file.write_all(body.as_bytes()).await?;
    Ok(ret_main)
}

mod check {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};

    pub struct Arg {
        pub client: reqwest::Client,
        pub ip: String,
        pub item: String,
    }
    pub struct Ret {
        pub item: Option<String>,
    }
    use json::{Json, By};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    pub async fn run(arg: Arg) -> Result<Ret> {
        let ip_eta = arg.ip.to_owned();
        match helper(&arg).await {
            Err(_err) => {
                // error!("item: {}, err: {:?}", arg.item, err);
                Ok(Ret{item: None})
            },
            Ok(ip) => {
                if ip == ip_eta {
                    Ok(Ret{item: None})
                } else {
                    Ok(Ret{item: Some(arg.item.to_owned())})
                }
            },
        }
    }
    pub async fn helper(arg: &Arg) -> Result<String> {
        let url = "https://bikuzin.baza-winner.ru/echo";
        let text = arg.client.get(url)
            .send()
            .await?
            .text()
            .await?
        ;
        let json = Json::from_str(text, url)?;
        let ip = json.get([By::key("headers"), By::key("x-real-ip")])?.as_string()?;
        Ok(ip)
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

    use pretty_assertions::{assert_eq};
    use term::Term;
    use std::time::{Instant, Duration};

    #[tokio::test]
    async fn test_get_anon() -> Result<()> {
        init();

        let list = read_list().await?;
        // info!("list: {:?}", list.len());
        let arg = Arg {
            thread_limit_network: 500,
            list,
        };
        let mut term = Term::init(term::Arg::new().header("Proxies . . ."))?;
        let start = Instant::now();
        let anon = get_anon(arg, Some(|arg: CallbackArg| -> Result<()> {
            term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}, found: {}", 
                arrange_millis::get(arg.elapsed_millis), 
                arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                arrange_millis::get(arg.remained_millis), 
                arrange_millis::get(arg.per_millis), 
                arg.elapsed_qt,
                arg.elapsed_qt + arg.remained_qt,
                arg.remained_qt,
                arg.ret_main_len,
            ))
        })).await?;
        println!("{}, Proxies got: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), anon.len());
        // info!("anon: {:?}", anon);

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_list() -> Result<()> {
        init();

        let lines = fetch_list().await?;
        info!("lines: {:?}", lines);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_list() -> Result<()> {
        init();

        let lines = read_list().await?;
        info!("lines: {:?}", lines);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_ip() -> Result<()> {
        init();

        let client = reqwest::Client::new();

        let ip = get_ip(client).await?;
        info!("ip: {}", ip);

        Ok(())
    }

}

