
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use lapin::{
    Consumer,
    options::{
        BasicRejectOptions,
    }, 
};
use std::time::Duration;
use futures::{StreamExt};
use super::super::rmq::{
    get_conn, 
    get_queue, 
    Pool, 
    basic_consume, 
    basic_publish, 
    basic_ack,
    basic_reject,
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
const RESPONSE_TIMEOUT: u64 = 5; //secs
const PROXY_REST_DURATION: u64 = 500; //millis
use super::super::req::Req;
use super::super::res::Res;

use futures::{
    future::FutureExt, // for `.fuse()`
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

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;
    let mut consumer_proxies_to_use_opt: Option<Consumer> = None;
    println!("{} connected, waiting for messages", consumer_tag.as_ref());
    let mut fut_queue = FuturesUnordered::new();
    loop {
        let next_fut = consumer.next().fuse();
        pin_mut!(next_fut);
        let mut consumer_next_fut = next_fut;
        select! {
            consumer_next_opt = consumer_next_fut => {
                if let Some(consumer_next) = consumer_next_opt {
                    if let Ok((channel, delivery)) = consumer_next {
                        let s = std::str::from_utf8(&delivery.data).unwrap();
                        let req: Req = serde_json::from_str(&s).unwrap();
                        let delivery_tag_req = delivery.delivery_tag;
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
                            let mut consumer_proxies_to_use = match consumer_proxies_to_use_opt {
                                Some(consumer_proxies_to_use) => consumer_proxies_to_use,
                                None => {
                                    let queue_name = "proxies_to_use";
                                    let consumer_tag = "consumer_proxies_to_use";
                                    basic_consume(&channel, queue_name, consumer_tag).await?
                                },
                            };
                            if let Some(consumer_proxies_to_use_next) = consumer_proxies_to_use.next().await {

                                if let Ok((channel, delivery)) = consumer_proxies_to_use_next {

                                    let line = std::str::from_utf8(&delivery.data).unwrap();
                                    trace!("line: {}, url: {}", line, req.url);
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
                                    let delivery_tag_proxy = delivery.delivery_tag;
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
                            consumer_proxies_to_use_opt = Some(consumer_proxies_to_use);
                        }
                    }
                }
            }
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => {
                        unreachable!();
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::Fetch(ret) => {
                                let fetch::Ret{ret, opt} = ret;
                                let fetch::Opt { req, delivery_tag_req, kind } = opt;
                                match ret {
                                    fetch::RequestRet::Err(err) => {
                                        match kind {
                                            fetch::Kind::NoProxy => {
                                                if err.is_timeout() {
                                                    warn!("is_timeout: {}", err);
                                                } else if err.is_builder() {
                                                    error!("is_builder: {}", err);
                                                    std::process::exit(1);
                                                } else if err.is_status() {
                                                    error!("is_status: {}, status: {:?}", err, err.status());
                                                    std::process::exit(1);
                                                } else if err.is_redirect() {
                                                    error!("is_redirect: {}, url: {:?}", err, err.url());
                                                    std::process::exit(1);
                                                } else {
                                                    error!("other: {:?}", err);
                                                    std::process::exit(1);
                                                }
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
                                                basic_ack(&channel, delivery_tag_proxy).await?;
                                                let _queue = get_queue(&channel, queue_name).await?;
                                                let proxy_error = ProxyError{line, msg};
                                                let payload = serde_json::to_string_pretty(&proxy_error)?;
                                                basic_publish(&channel, queue_name, payload).await?;
                                                basic_reject(&channel, delivery_tag_req).await?;
                                            },
                                        }
                                    },
                                    fetch::RequestRet::Ok{url, text, status} => {
                                        match kind {
                                            fetch::Kind::NoProxy => {
                                                let url_res = url;
                                                info!("{}: {}", req.url, status);
                                                let res = Res {
                                                    url_req: req.url,
                                                    url_res,
                                                    status,
                                                    text,
                                                };

                                                let queue_name: &str = req.reply_to.as_ref();
                                                let _queue = get_queue(&channel, queue_name).await?;
                                                basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                                basic_ack(&channel, delivery_tag_req).await?;
                                            },
                                            fetch::Kind::Proxy { line, delivery_tag_proxy }=> {
                                                if status == http::StatusCode::FORBIDDEN {
                                                    warn!("{}: forbidden", line);
                                                    basic_ack(&channel, delivery_tag_proxy).await?;
                                                    let queue_name = "proxies_forbidden";
                                                    let _queue = get_queue(&channel, queue_name).await?;
                                                    basic_publish(&channel, queue_name, line).await?;
                                                    basic_reject(&channel, delivery_tag_req).await?;
                                                } else {
                                                    let url_res = url;
                                                    info!("{}: {}", req.url, status);
                                                    let res = Res {
                                                        url_req: req.url,
                                                        url_res,
                                                        status,
                                                        text,
                                                    };

                                                    let queue_name: &str = req.reply_to.as_ref();
                                                    let _queue = get_queue(&channel, queue_name).await?;
                                                    basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                                    basic_ack(&channel, delivery_tag_req).await?;
                                                    tokio::time::delay_for(Duration::from_millis(PROXY_REST_DURATION)).await;

                                                    info!("line: {}", line);
                                                    channel.basic_reject(delivery_tag_proxy, BasicRejectOptions{ requeue: true }).await?;
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
            complete => {
                break;
            },
        }
    }
    Ok(())
}

enum OpArg {
    Fetch(fetch::Arg),
}

enum OpRet {
    Fetch(fetch::Ret),
}

async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::Fetch(arg) => {
            let ret = fetch::run(arg).await?;
            Ok(OpRet::Fetch(ret))
        },
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

