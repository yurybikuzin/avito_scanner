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
// use tokio::fs::File;

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "fetch_proxies";
        let consumer_name = "fetch_proxies_consumer";
        println!("connecting {} ...", consumer_name);
        match listen(pool.clone(), consumer_name, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_name),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_name, e),
        };
    }
}

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_name: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_name.as_ref()).await?;
    println!("{} connected, waiting for messages", consumer_name.as_ref());
    while let Some(delivery) = consumer.next().await {
        if let Ok((channel, delivery)) = delivery {

            println!("received msg: {:?}", delivery);

            let body = reqwest::get("http://rootjazz.com/proxies/proxies.txt")
                .await?
                .text()
                .await?
            ;
            let lines = body.lines().map(|s| s.to_owned()).collect::<Vec<String>>();
            let queue_name = "proxies_to_check";
            for line in lines {
                basic_publish(&channel, queue_name, line).await?;
            }

            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await?
        }
    }
    Ok(())
}
