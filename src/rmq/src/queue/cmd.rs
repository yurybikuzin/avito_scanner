
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

// use lapin::{
//     options::{
//         BasicAckOptions, 
//         // BasicRejectOptions,
//         // BasicNackOptions,
//     }, 
// };
use std::time::Duration;
use futures::{StreamExt};
use super::super::rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish, basic_ack};
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
                    let body = reqwest::get("http://rootjazz.com/proxies/proxies.txt")
                        .await?
                        .text()
                        .await?
                    ;
                    let lines = body.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
                    let queue_name = "proxies_to_check";
                    let _queue = get_queue(&channel, queue_name).await?;
                    for line in lines {
                        basic_publish(&channel, queue_name, line).await?;
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
