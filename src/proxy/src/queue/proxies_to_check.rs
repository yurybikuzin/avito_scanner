
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
            Ok(_) => warn!("{} listen returned", consumer_tag),
            Err(e) => error!("{} listen had an error: {}", consumer_tag, e),
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
        FutureExt,// for `.fuse()`
    }, 
    select,
    pin_mut,
    stream::{
        FuturesUnordered,
    },
};

pub struct Checked {
    pub status: check::Status,
    pub url: String,
}

use std::collections::HashMap;

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

    let mut checked = HashMap::<String, Vec<Checked>>::new();
    let protos = vec!["socks5", "http", "https"];

    loop {
        let same_time_proxy_check_max = SAME_TIME_PROXY_CHECK_MAX.load(Ordering::Relaxed) as usize;
        let consumer_next_fut = if fut_queue.len() < same_time_proxy_check_max - protos.len() {
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
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => unreachable!(),
                    Ok(ret) => {
                        match ret {
                            OpRet::Check(check::Ret{status, opt}) => {
                                let check::Opt { host, url, delivery_tag } = opt;
                                let mut vec_checked = match checked.get_mut(&host) {
                                    Some(vec_checked) => vec_checked,
                                    None => {
                                        let mut vec_checked = vec![];
                                        checked.insert(host.clone(), vec_checked);
                                        checked.get_mut(&host).unwrap()
                                    },
                                };
                                match &status {
                                    check::Status::Ok(delay) => {
                                        info!("{}: Ok, {}", url, arrange_millis::get(*delay));
                                    },
                                    check::Status::NonAnon => {
                                        warn!("{}: alive but is not ANON", url);
                                    },
                                    check::Status::Err(err) => {
                                        // trace!("{}: {}", url, err);
                                    },
                                }
                                vec_checked.push(Checked {
                                    status,
                                    url,
                                });
                                if vec_checked.len() >= protos.len() {
                                    for item in vec_checked.iter() {
                                        match item.status {
                                            check::Status::Ok(_) => {
                                                let queue_name = "proxies_to_use";
                                                let _queue = get_queue(&channel, queue_name).await?;
                                                let url = item.url.clone();
                                                basic_publish(&channel, queue_name, &url).await
                                                    .context(format!("basic_publish({}, {})", queue_name, url))
                                                ?;
                                                break;
                                            },
                                            _ => {},
                                        }
                                    }
                                    checked.remove(&host);
                                    basic_ack(&channel, delivery_tag).await
                                        .map_err(|err| anyhow!(err))
                                        .context(format!("basic_ack({})", delivery_tag))
                                    ?;
                                }
                            },
                        }
                    }
                }
            },
            consumer_next_opt = consumer_next_fut => {
                if let Some(consumer_next) = consumer_next_opt {
                    if let Ok((channel, delivery)) = consumer_next {
                        let own_ip = match own_ip_opt {
                            None => get_own_ip().await?,
                            Some(own_ip) => {
                                if Instant::now().duration_since(own_ip.last_update).as_secs() < OWN_IP_FRESH_DURATION.load(Ordering::Relaxed) as u64 {
                                    own_ip
                                } else {
                                    match get_own_ip().await {
                                        Ok(own_ip) => own_ip,
                                        Err(err) => {
                                            warn!("old own_ip {} will be used due to {}", own_ip.ip, err);
                                            own_ip
                                        },
                                    }
                                }
                            },
                        };

                        let host = std::str::from_utf8(&delivery.data).unwrap();
                        for proto in &protos {
                            let url = format!("{}://{}", proto, host);
                            let proxy = match reqwest::Proxy::all(&url) {
                                Ok(url) => url,
                                Err(err) => {
                                    warn!("malformed url {}: {}", url, err);
                                    break;
                                },
                            };
                            let client = reqwest::Client::builder()
                                .proxy(proxy)
                                .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
                                .build()?
                            ;
                            let opt = check::Opt {
                                url,
                                host: host.to_owned(), 
                                delivery_tag: delivery.delivery_tag,
                            };
                            fut_queue.push(op(OpArg::Check(check::Arg { 
                                client, 
                                own_ip: own_ip.ip.to_owned(), 
                                opt,
                            })));
                            STATE_PROXIES_TO_USE.store(STATE_PROXIES_TO_USE_FILLED, Ordering::Relaxed);
                        }

                        own_ip_opt = Some(own_ip);
                    } else {
                        error!("Err(err) = consumer_next");
                    }
                } else {
                    error!("consumer_next_opt.is_none()");
                }
            },
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
        pub own_ip: String,
        pub opt: Opt,
    }

    pub struct Ret {
        pub status: Status,
        pub opt: Opt,
    }

    pub struct Opt {
        pub url: String,
        pub host: String,
        pub delivery_tag: amq_protocol_types::LongLongUInt,
    }

    pub enum Status {
        Ok(u128),
        NonAnon,
        Err(Error),
    }

    pub async fn run(arg: Arg) -> Result<Ret> {
        let start = std::time::Instant::now();
        let status = match super::get_ip(arg.client).await {
            Err(err) => Status::Err(err),
            Ok(ip) => if ip != arg.own_ip {
                Status::Ok(std::time::Instant::now().duration_since(start).as_millis())
            } else {
                Status::NonAnon
            },
        };
        Ok(Ret{
            status,
            opt: arg.opt,
        })
    }
}

pub async fn get_own_ip() -> Result<OwnIp> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
        .build()?
    ;
    let ip = get_ip( client).await?;
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
