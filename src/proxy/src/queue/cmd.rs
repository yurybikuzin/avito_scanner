
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use lapin::{
    Channel,
//     options::{
//         BasicAckOptions, 
//         // BasicRejectOptions,
//         // BasicNackOptions,
//     }, 
};
use std::time::Duration;
use futures::{StreamExt};
use rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish, basic_ack};
// use tokio::fs::File;

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "cmd";
        let consumer_tag = "cmd_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_tag),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_tag, e),
        };
    }
}

use serde::Serialize;
#[derive(Serialize)]
struct CmdError<'a> {
    cmd: &'a str,
    msg: String,
}

use super::*;

pub async fn fetch_proxies(channel: &Channel) -> Result<()> {
    if 
        STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_NONE ||
        STATE_PROXIES_TO_USE.load(Ordering::Relaxed) == STATE_PROXIES_TO_USE_NONE ||
        false
    {
        let queue_name = "cmd";
        let _queue = get_queue(channel, queue_name).await?;
        basic_publish(channel, queue_name, "fetch_proxies").await?;
    } else {
        trace!("cmd fetch_proxies ignored due to STATE_PROXIES_TO_CHECK/USE is not NONE");
    }
    Ok(())
}

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;
    println!("{} connected, waiting for messages", consumer_tag.as_ref());
    while let Some(delivery) = consumer.next().await {
        if let Ok((channel, delivery)) = delivery {

            let cmd = std::str::from_utf8(&delivery.data).unwrap();
            match cmd {
                "fetch_proxies" => {
                    if STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_NONE {
                        STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_STARTED, Ordering::Relaxed);
                        let queue_name = "proxies_to_check";

                        let body = reqwest::get("http://rootjazz.com/proxies/proxies.txt")
                            .await?
                            .text()
                            .await?
                        ;
                        let lines = body.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
                        for line in lines {
                            let url_proxy = format!("http://{}", line);
                            if let Ok(_url_proxy) = reqwest::Proxy::all(&url_proxy) {
                                basic_publish(&channel, queue_name, line).await?;
                                STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_FILLED, Ordering::Relaxed);
                            }
                        }
                        if STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_STARTED {
                            STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_NONE, Ordering::Relaxed);
                        }
                    } else {
                        trace!("cmd fetch_proxies ignored due to STATE_PROXIES_TO_CHECK/USE is not NONE");
                    }
                },
                _ => {
                    let queue_name = "cmd_error";
                    let _queue = get_queue(&channel, queue_name).await?;
                    let msg = format!("available commands: 'fetch_proxies'");
                    warn!("unknown command '{}', {}", cmd, msg);
                    let cmd_error = CmdError { cmd, msg };
                    let payload = serde_json::to_string_pretty(&cmd_error)?;
                    basic_publish(&channel, queue_name, payload).await?;
                },
            }
            basic_ack(&channel, delivery.delivery_tag).await?;
        }
    }
    Ok(())
}
