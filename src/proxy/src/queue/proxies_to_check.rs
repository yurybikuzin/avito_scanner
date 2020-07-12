
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::time::Duration;
use futures::{StreamExt};
use rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish, basic_ack};
use json::{Json, By};

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "proxies_to_check";
        let consumer_tag = "proxies_to_check_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_tag),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_tag, e),
        };
    }
}

use std::time::Instant;
pub struct OwnIp {
    ip: String,
    last_update: Instant,
}
use futures::{
    future::{
        Fuse, 
        // FusedFuture,
        FutureExt,// for `.fuse()`
    }, 
    select,
    pin_mut,
    stream::{
        FuturesUnordered,
    },
    // channel::mpsc,
};

use super::*;
async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;
    println!("{} connected, waiting for messages", consumer_tag.as_ref());

    let mut fut_queue = FuturesUnordered::new();
    let mut own_ip_opt: Option<OwnIp> = None;

    // let (sender, mut stream) = mpsc::unbounded();
    loop {
        let same_time_proxy_check_max = SAME_TIME_PROXY_CHECK_MAX.load(Ordering::Relaxed) as usize;
        let consumer_next_fut = if fut_queue.len() <  same_time_proxy_check_max{
            consumer.next().fuse()
        } else {
            Fuse::terminated()
        };
        let consumer_timeout_fut = if fut_queue.len() < same_time_proxy_check_max && STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_FILLED {
            tokio::time::delay_for(std::time::Duration::from_secs(5)).fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_timeout_fut);
        pin_mut!(consumer_next_fut);
        select! {
            ret = consumer_timeout_fut => {
                STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_NONE, Ordering::Relaxed);
            },
            consumer_next_opt = consumer_next_fut => {
                if let Some(consumer_next) = consumer_next_opt {
                    if let Ok((channel, delivery)) = consumer_next {
                        let line = std::str::from_utf8(&delivery.data).unwrap();
                        let url_proxy = format!("http://{}", line);
                        match reqwest::Proxy::all(&url_proxy) {
                            Err(_err) => {
                                basic_ack(&channel, delivery.delivery_tag).await?;
                            },
                            Ok(url_proxy) => {
                                let own_ip = match own_ip_opt {
                                    None => get_own_ip().await?,
                                    Some(own_ip) => {
                                        if Instant::now().duration_since(own_ip.last_update).as_secs() < OWN_IP_FRESH_DURATION.load(Ordering::Relaxed) as u64 {
                                            own_ip
                                        } else {
                                            get_own_ip().await?
                                        }
                                    },
                                };
                                let client = reqwest::Client::builder()
                                    .proxy(url_proxy)
                                    .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
                                    .build()?
                                ;
                                fut_queue.push(op(OpArg::Check(check::Arg {
                                    client, 
                                    line: line.to_owned(), 
                                    own_ip: own_ip.ip.to_owned(),
                                    delivery_tag: delivery.delivery_tag,
                                })));
                                STATE_PROXIES_TO_USE.store(STATE_PROXIES_TO_USE_FILLED, Ordering::Relaxed);
                                own_ip_opt = Some(own_ip);
                            },
                        };
                    } else {
                        error!("Err(err) = consumer_next");
                    }
                } else {
                    error!("consumer_next_opt.is_none()");
                }
            },
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => unreachable!(),
                    Ok(ret) => {
                        match ret {
                            OpRet::Check(check::Ret{line, delivery_tag}) => {
                                if let Some(line) = line {
                                    let queue_name = "proxies_to_use";
                                    let _queue = get_queue(&channel, queue_name).await?;
                                    basic_publish(&channel, queue_name, line).await?;
                                }
                                basic_ack(&channel, delivery_tag).await?;
                            },
                        }
                    }
                }
            },
            // () = stream.select_next_some() => { 
            //     error!("unreachable stream.select_next_some");
            // },
            complete => {
                error!("unreachable complete");
                break;
            }
        }
    }
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
        pub delivery_tag: amq_protocol_types::LongLongUInt,
    }

    pub struct Ret {
        pub line: Option<String>,
        pub delivery_tag: amq_protocol_types::LongLongUInt,
    }

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
        Ok(Ret{
            line,
            delivery_tag: arg.delivery_tag,
        })
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
