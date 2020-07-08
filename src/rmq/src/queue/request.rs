
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::collections::VecDeque;
// use lapin::{
//     // Consumer,
//     // options::{
//     //     BasicRejectOptions,
//     // }, 
// };
use std::time::Duration;
use futures::{StreamExt};
use super::super::rmq::{
    get_conn, 
    get_queue, 
    Pool, 
    basic_consume, 
    basic_publish, 
    basic_ack,
    // basic_reject,
};

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "request";
        let consumer_tag = "request_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_tag),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_tag, e),
        };
    }
}

const RESPONSE_TIMEOUT: u64 = 10; //secs
const PROXY_REST_DURATION: u64 = 1000; //millis
// const FETCH_PROXIES_AFTER_DURATION: u64 = 5; //secs
use super::super::req::Req;
use super::super::res::Res;

use futures::{
    future::{
        Fuse,
        FutureExt, // for `.fuse()`
    },
    select,
    pin_mut,
    stream::{
        FuturesUnordered,
    },
};

use serde::Serialize;
#[derive(Serialize)]
struct ProxyError {
    line: String,
    msg: String,
}

use super::*;
const SAME_TIME_REQUEST_MAX: usize = 20;

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer_request = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;

    let mut consumer_proxies_to_use = {
        let queue_name = "proxies_to_use";
        let consumer_tag = "consumer_proxies_to_use";
        let _queue = get_queue(&channel, queue_name).await?;
        basic_consume(&channel, queue_name, consumer_tag).await?
    };

    // let mut consumer_proxies_to_use_opt: Option<Consumer> = None;
    //
    println!("{} connected, waiting for messages", consumer_tag.as_ref());
    let mut fut_queue = FuturesUnordered::new();
    let mut reqs: VecDeque<(Req, amq_protocol_types::LongLongUInt)> = VecDeque::new();
    let mut lines: VecDeque<(String, amq_protocol_types::LongLongUInt)> = VecDeque::new();
    loop {
        if reqs.len() > 0 && lines.len() > 0 {
            let (line, delivery_tag_proxy) = lines.pop_front().unwrap();
            let (req, delivery_tag_req) = reqs.pop_front().unwrap();
            let no_proxy = req.no_proxy.unwrap_or(false); 
            if no_proxy {
                let client = reqwest::Client::builder()
                    .timeout(
                        req.timeout.unwrap_or(
                            Duration::from_secs(RESPONSE_TIMEOUT)
                        )
                    )
                    .build()?
                ;
                fut_queue.push(op(OpArg::Fetch(fetch::Arg {
                    client, 
                    opt: fetch::Opt {
                        req,
                        delivery_tag_req,
                        kind: fetch::Kind::NoProxy,
                    },
                })));
            } else {
                let url_proxy = format!("http://{}", line);
                let url_proxy = reqwest::Proxy::all(&url_proxy).unwrap();
                let client = reqwest::Client::builder()
                    .proxy(url_proxy)
                    .timeout(
                        req.timeout.unwrap_or(
                            Duration::from_secs(RESPONSE_TIMEOUT)
                        )
                    )
                    .build()?
                ;
                trace!("took {} for {}", line, req.url);
                fut_queue.push(op(OpArg::Fetch(fetch::Arg {
                    client, 
                    opt: fetch::Opt {
                        req,
                        delivery_tag_req,
                        kind: fetch::Kind::Proxy {
                            delivery_tag_proxy,
                            line: line.to_owned(),
                        },
                    },
                })));
            }
        }

        let consumer_request_next_fut = if fut_queue.len() < SAME_TIME_REQUEST_MAX {
            consumer_request.next().fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_request_next_fut);

        let consumer_proxies_to_use_next_fut = if lines.len() < reqs.len() {
            super::cmd::fetch_proxies(&channel).await?;
            consumer_proxies_to_use.next().fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_proxies_to_use_next_fut);

        let consumer_timeout_fut = if lines.len() < reqs.len() && STATE_PROXIES_TO_USE.load(Ordering::Relaxed) == STATE_PROXIES_TO_USE_FILLED {
            tokio::time::delay_for(std::time::Duration::from_secs(5)).fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_timeout_fut);

        select! {
            ret = consumer_timeout_fut => {
                STATE_PROXIES_TO_USE.store(STATE_PROXIES_TO_USE_NONE, Ordering::Relaxed);
            },
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => {
                        unreachable!();
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::ReuseProxy(ret) => {
                                let reuse_proxy::Ret { line, delivery_tag } = ret;
                                info!("{} to be reused", line);
                                lines.push_front((line, delivery_tag));
                            },
                            OpRet::Fetch(ret) => {
                                let fetch::Ret{ret, opt} = ret;
                                let fetch::Opt { req, delivery_tag_req, kind } = opt;
                                match ret {
                                    fetch::RequestRet::Err(err) => {
                                        match kind {
                                            fetch::Kind::NoProxy => {
                                                todo!();
                                                // if err.is_timeout() {
                                                //     warn!("is_timeout: {}", err);
                                                // } else if err.is_builder() {
                                                //     error!("is_builder: {}", err);
                                                //     std::process::exit(1);
                                                // } else if err.is_status() {
                                                //     error!("is_status: {}, status: {:?}", err, err.status());
                                                //     std::process::exit(1);
                                                // } else if err.is_redirect() {
                                                //     error!("is_redirect: {}, url: {:?}", err, err.url());
                                                //     std::process::exit(1);
                                                // } else {
                                                //     error!("other: {:?}", err);
                                                //     std::process::exit(1);
                                                // }
                                            },
                                            fetch::Kind::Proxy { delivery_tag_proxy, line } => {
                                                let (queue_name, msg) =  if err.is_timeout() {
                                                    ("proxies_error_timeout", format!("is_timeout: {}", err))
                                                } else if err.is_builder() {
                                                    ("proxies_error_builder", format!("is_builder({}): {}", line, err))
                                                } else if err.is_status() {
                                                    ("proxies_error_status", format!("is_status({}): {}, status: {:?}", line, err, err.status()))
                                                } else if err.is_redirect() {
                                                    ("proxies_error_redirect", format!("is_redirect({}): {}, url: {:?}", line, err, err.url()))
                                                } else {
                                                    ("proxies_error_other", format!("other({}): {}", line, err))
                                                };
                                                warn!("{}: {}", line, msg);
                                                let _queue = get_queue(&channel, queue_name).await?;
                                                let proxy_error = ProxyError{line, msg};
                                                let payload = serde_json::to_string_pretty(&proxy_error)?;
                                                basic_publish(&channel, queue_name, payload).await?;
                                                basic_ack(&channel, delivery_tag_proxy).await?;
                                                reqs.push_front((req, delivery_tag_req));
                                            },
                                        }
                                    },
                                    fetch::RequestRet::Ok{url, text, status} => {
                                        match kind {
                                            fetch::Kind::NoProxy => {
                                                todo!();
                                                // let url_res = url;
                                                // info!("{}: {}", req.url, status);
                                                // let res = Res {
                                                //     url_req: req.url,
                                                //     url_res,
                                                //     status,
                                                //     text,
                                                // };
                                                //
                                                // let queue_name: &str = req.reply_to.as_ref();
                                                // let _queue = get_queue(&channel, queue_name).await?;
                                                // basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;
                                                //
                                                // basic_ack(&channel, delivery_tag_req).await?;
                                            },

                                            fetch::Kind::Proxy { line, delivery_tag_proxy }=> {
                                                let url_res = url;
                                                info!("{}: {}: {}", line, req.url, status);
                                                let res = Res {
                                                    url_req: req.url.clone(),
                                                    url_res,
                                                    status,
                                                    text,
                                                };
                                                match status {
                                                    http::StatusCode::OK => {

                                                        let queue_name: &str = req.reply_to.as_ref();
                                                        let _queue = get_queue(&channel, queue_name).await?;
                                                        basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                                        basic_ack(&channel, delivery_tag_req).await?;
                                                        fut_queue.push(op(OpArg::ReuseProxy(reuse_proxy::Arg {
                                                            line,
                                                            delivery_tag: delivery_tag_proxy,
                                                        })));
                                                    },
                                                    _ => {
                                                        warn!("{}: {:?}", line, status);
                                                        let queue_name = format!("proxies_status_{:?}", status);
                                                        let _queue = get_queue(&channel, queue_name.as_str(),).await?;
                                                        basic_publish(&channel, queue_name.as_str(), line).await?;
                                                        basic_ack(&channel, delivery_tag_proxy).await?;
                                                        reqs.push_front((req, delivery_tag_req));
                                                    },
                                                }
                                            },
                                        }
                                    }
                                }
                            },
                        }
                    },
                }
            },
            consumer_proxies_to_use_next_opt = consumer_proxies_to_use_next_fut => {
                if let Some(consumer_proxies_to_use_next) = consumer_proxies_to_use_next_opt {
                    if let Ok((channel, delivery)) = consumer_proxies_to_use_next {
                        let line = std::str::from_utf8(&delivery.data).unwrap();
                        let url_proxy = format!("http://{}", line);
                        if let Ok(_url_proxy) = reqwest::Proxy::all(&url_proxy) {
                            lines.push_back((line.to_owned(), delivery.delivery_tag));
                        }
                    } else {
                        error!("Err(err) = consumer_proxies_to_use_next");
                    }
                } else {
                    error!("consumer_proxies_to_use_next_opt.is_none()");
                }
            },
            consumer_request_next_opt = consumer_request_next_fut => {
                if let Some(consumer_request_next) = consumer_request_next_opt {
                    if let Ok((channel, delivery)) = consumer_request_next {
                        let s = std::str::from_utf8(&delivery.data).unwrap();
                        let req: Req = serde_json::from_str(&s).unwrap();
                        reqs.push_back((req, delivery.delivery_tag));
                    } else {
                        error!("Err(err) = consumer_request_next");
                    }
                } else {
                    error!("consumer_request_next_opt.is_none()");
                }
            },
            complete => {
                error!("complete: unreachable!");
                break;
            },
        }
    }
    Ok(())
}

enum OpArg {
    Fetch(fetch::Arg),
    ReuseProxy(reuse_proxy::Arg),
}

enum OpRet {
    Fetch(fetch::Ret),
    ReuseProxy(reuse_proxy::Ret),
}

async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::Fetch(arg) => {
            let ret = fetch::run(arg).await?;
            Ok(OpRet::Fetch(ret))
        },
        OpArg::ReuseProxy(arg) => {
            let ret = reuse_proxy::run(arg).await?;
            Ok(OpRet::ReuseProxy(ret))
        },
    }
}

mod reuse_proxy {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};

    pub struct Arg {
        pub line: String,
        pub delivery_tag: amq_protocol_types::LongLongUInt,
    }

    pub struct Ret {
        pub line: String,
        pub delivery_tag: amq_protocol_types::LongLongUInt,
    }

    use std::time::Duration;
    pub async fn run(arg: Arg) -> Result<Ret> {
        tokio::time::delay_for(Duration::from_millis(super::PROXY_REST_DURATION)).await;
        Ok(Ret {
            line: arg.line,
            delivery_tag: arg.delivery_tag,
        })
    }
}

mod fetch {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use amq_protocol_types::LongLongUInt;

    use super::{Req};

    pub struct Arg {
        pub client: reqwest::Client,
        pub opt: Opt,
    }

    pub struct Ret {
        pub ret: RequestRet,
        pub opt: Opt,
    }

    pub enum RequestRet {
        Err(reqwest::Error),
        Ok{
            url: reqwest::Url,
            status: http::StatusCode,
            text: String,
        },
    }

    pub struct Opt {
        pub req: Req,
        pub delivery_tag_req: LongLongUInt,
        pub kind: Kind,
    }

    pub enum Kind {
        NoProxy,
        Proxy {
            delivery_tag_proxy: LongLongUInt,
            line: String,
        },
    }

    pub async fn run(arg: Arg) -> Result<Ret> {
        let Arg {  client, opt } = arg;
        match client.request(opt.req.method.clone(), opt.req.url.clone()).send().await {
            Err(err) => Ok(Ret { 
                ret: RequestRet::Err(err),
                opt,
            }),
            Ok(response) => {
                let url = response.url().clone();
                let status = response.status();
                match response.text().await {
                    Err(err) => Ok(Ret { 
                        ret: RequestRet::Err(err),
                        opt,
                    }),
                    Ok(text) => Ok(Ret {
                        ret: RequestRet::Ok{
                            url,
                            status,
                            text,
                        },
                        opt,
                    })
                }
            }
        }
    }
}

