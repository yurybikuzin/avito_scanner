
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use lapin::{
    Consumer,
    options::{
        BasicAckOptions, 
        BasicRejectOptions,
        // BasicNackOptions,
    }, 
};
use std::time::Duration;
use futures::{StreamExt};
use super::super::rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish };

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
    while let Some(consumer_next) = consumer.next().await {
        if let Ok((channel, delivery)) = consumer_next {
            let s = std::str::from_utf8(&delivery.data).unwrap();
            let req: Req = serde_json::from_str(&s).unwrap();
            let req_delivery_tag = delivery.delivery_tag;
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
                match client.request(req.method, req.url.clone()).send().await {
                    Err(err) => {
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
                    Ok(response) => {
                        let status = response.status();
                        info!("{}: {}", req.url, status);
                    },
                }
                channel
                    .basic_ack(req_delivery_tag, BasicAckOptions::default())
                    .await?
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
                        trace!("line: {}", line);
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
                        match client.request(req.method, req.url.clone()).send().await {
                            Err(err) => {
                                if err.is_timeout() {
                                    warn!("is_timeout: {}", err);
                                    channel
                                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                                        .await?
                                    ;
                                    let queue_name = "proxies_timeout";
                                    let _queue = get_queue(&channel, queue_name).await?;
                                    basic_publish(&channel, queue_name, line).await?;
                                    channel.basic_reject(req_delivery_tag, BasicRejectOptions{ requeue: true }).await?;

                                } else if err.is_builder() {
                                    error!("is_builder({}): {}", line, err);
                                    std::process::exit(1);
                                } else if err.is_status() {
                                    error!("is_status({}): {}, status: {:?}", line, err, err.status());
                                    std::process::exit(1);
                                } else if err.is_redirect() {
                                    error!("is_redirect({}): {}, url: {:?}", line, err, err.url());
                                    std::process::exit(1);
                                } else {
                                    error!("other({}): {}", line, err);
                                    std::process::exit(1);
                                }
                            },
                            Ok(response) => {
                                let status = response.status();
                                if status == http::StatusCode::FORBIDDEN {
                                    warn!("forbidden {}", line);
                                    channel
                                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                                        .await?
                                    ;
                                    let queue_name = "proxies_forbidden";
                                    let _queue = get_queue(&channel, queue_name).await?;
                                    basic_publish(&channel, queue_name, line).await?;
                                    channel.basic_reject(req_delivery_tag, BasicRejectOptions{ requeue: true }).await?;
                                } else {
                                    let text = response.text().await?;
                                    info!("{}: {}", req.url, status);
                                    let res = Res {
                                        status,
                                        text,
                                    };

                                    let queue_name: &str = req.reply_to.as_ref();
                                    let _queue = get_queue(&channel, queue_name).await?;
                                    basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                    channel.basic_ack(req_delivery_tag, BasicAckOptions::default()).await?;
                                    tokio::time::delay_for(Duration::from_millis(PROXY_REST_DURATION)).await;
                                    
                                    channel.basic_reject(delivery.delivery_tag, BasicRejectOptions{ requeue: true }).await?;
                                }
                            },
                        }
                    }
                }
                consumer_proxies_to_use_opt = Some(consumer_proxies_to_use);
            }
        }
    }
    Ok(())
}
