
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use lapin::{
    options::{
        BasicAckOptions, 
        // BasicRejectOptions,
        // BasicNackOptions,
    }, 
};
use std::time::Duration;
use futures::{StreamExt};
use super::super::rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish };
use json::{Json, By};

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "proxies_to_check";
        let consumer_name = "proxies_to_check_consumer";
        println!("connecting {} ...", consumer_name);
        match listen(pool.clone(), consumer_name, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_name),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_name, e),
        };
    }
}

use std::time::Instant;
pub struct OwnIp {
    ip: String,
    last_update: Instant,
}
const PROXY_TIMEOUT: u64 = 10;//secs
const OWN_IP_FRESH_DURATION: u64 = 30;//secs

use futures::{
    future::FutureExt, // for `.fuse()`
    select,
    pin_mut,
    stream::{
        // StreamExt,
        FuturesUnordered,
    },
};
async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_name: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_name.as_ref()).await?;
    println!("{} connected, waiting for messages", consumer_name.as_ref());
    // let mut url_proxies: Vec<reqwest::Proxy> = Vec::new();
    // let mut delay: Option<tokio::time::Delay> = None;
    let mut fut_queue = FuturesUnordered::new();
    let mut own_ip_opt: Option<OwnIp> = None;
    loop {
        let next_fut = consumer.next().fuse();
        pin_mut!(next_fut);
        let mut consumer_next_fut = next_fut;
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => unreachable!(),
                    Ok(ret) => {
                        match ret {
                            OpRet::Check(check::Ret{line}) => {
                                if let Some(line) = line {
                                    let queue_name = "proxies_to_use";
                                    let _queue = get_queue(&channel, queue_name).await?;
                                    basic_publish(&channel, queue_name, line).await?;
                                }
                            },
                        }
                    }
                }
            },
            consumer_next_opt = consumer_next_fut => {
                if let Some(consumer_next) = consumer_next_opt {
                    if let Ok((channel, delivery)) = consumer_next {
                        let line = std::str::from_utf8(&delivery.data).unwrap();
                        let url_proxy = format!("http://{}", line);

                        match reqwest::Proxy::all(&url_proxy) {
                            Err(_err) => {
                                // error!("{}: {}", url, err);
                            },
                            Ok(url_proxy) => {
                                let own_ip = match own_ip_opt {
                                    None => get_own_ip().await?,
                                    Some(own_ip) => {
                                        if Instant::now().duration_since(own_ip.last_update).as_secs() < OWN_IP_FRESH_DURATION {
                                            own_ip
                                        } else {
                                            get_own_ip().await?
                                        }
                                    },
                                };
                                let client = reqwest::Client::builder()
                                    .proxy(url_proxy)
                                    .timeout(Duration::from_secs(PROXY_TIMEOUT))
                                    .build()?
                                ;
                                fut_queue.push(op(OpArg::Check(check::Arg {client, line: line.to_owned(), own_ip: own_ip.ip.to_owned()})));
                                own_ip_opt = Some(own_ip);
                            },
                        };
                        channel
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .await?;
                        let next_fut = consumer.next().fuse();
                        pin_mut!(next_fut);
                        consumer_next_fut = next_fut;
                    }
                }
            },
            complete => {
                break;
            },
        }
    }
    // while let Some(delivery) = consumer.next().await {
    //     // let queue = get_queue(&channel, queue_name.as_ref()).await?;
    //     // info!("message_count: {}", queue.message_count());
    //
    //     if let Ok((channel, delivery)) = delivery {
    //         let line = std::str::from_utf8(&delivery.data).unwrap();
    //         let url_proxy = format!("http://{}", line);
    //         match reqwest::Proxy::all(&url_proxy) {
    //             Err(_err) => {
    //                 // error!("{}: {}", url, err);
    //             },
    //             Ok(url_proxy) => {
    //                 url_proxies.push(url_proxy);
    //                 info!("len: {}", url_proxies.len());
    //         //         let client = reqwest::Client::builder()
    //         //             .proxy(url_proxy)
    //         //             .timeout(Duration::from_secs(10))
    //         //             .build()?
    //         //         ;
    //         //         // match get_ip(client).await {
    //         //         //     Err(_err) => {
    //         //         //         // error!("item: {}, err: {:?}", arg.item, err);
    //         //         //         Ok(Ret{item: None})
    //         //         //     },
    //         //         //     Ok(ip) => {
    //         //         //         if ip == ip_eta {
    //         //         //             Ok(Ret{item: None})
    //         //         //         } else {
    //         //         //             Ok(Ret{item: Some(arg.item.to_owned())})
    //         //         //         }
    //         //         //     },
    //         //         // }
    //         //         // let url = "https://bikuzin.baza-winner.ru/echo";
    //         //         // let text = client.get(url)
    //         //         //     .send()
    //         //         //     .await?
    //         //         //     .text()
    //         //         //     .await?
    //         //         // ;
    //         //         // let json = Json::from_str(text, url)?;
    //         //         // let ip = json.get([By::key("headers"), By::key("x-real-ip")])?.as_string()?;
    //         //         // let arg = OpArg::Check (check::Arg {
    //         //         //     ip: $ip.to_owned(),
    //         //         //     client,
    //         //         //     item,
    //         //         // });
    //         //         // let fut = op(arg);
    //         //         // $fut_queue.push(fut);
    //         //         // $used_network_threads += 1;
    //             },
    //         };
    //         // let client = reqwest::Client::builder()
    //         //     .proxy()
    //         //     .timeout(Duration::from_secs(10))
    //         //     .build()?
    //         // ;
    //         //
    //
    //         channel
    //             .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
    //             .await?
    //     }
    // }
    Ok(())
}

enum OpArg {
    Check(check::Arg),
}

enum OpRet {
    Check(check::Ret),
}

async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::Check(arg) => {
            let ret = check::run(arg).await?;
            Ok(OpRet::Check(ret))
        },
    }
}

mod check {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};

    pub struct Arg {
        pub client: reqwest::Client,
        pub line: String,
        pub own_ip: String,
    }

    pub struct Ret {
        pub line: Option<String>,
    }
                                    // fut_queue.push(check(CheckArg {client, line, own_ip.unwrap().ip}));
    pub async fn run(arg: Arg) -> Result<Ret> {
        let line = match super::get_ip(arg.client).await {
            Err(_) => None,
            Ok(ip) => {
                if ip == arg.own_ip {
                    None
                } else {
                    Some(arg.line)
                }
            },
        };
        Ok(Ret{line})
    }
}

pub async fn get_own_ip() -> Result<OwnIp> {
    let ip = get_ip(reqwest::Client::new()).await?;
    Ok(OwnIp{
        ip, 
        last_update: Instant::now(),
    })
}

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
