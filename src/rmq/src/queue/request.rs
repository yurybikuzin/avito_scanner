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
use super::super::rmq::{get_conn, get_queue, Pool, basic_consume };

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "request";
        let consumer_name = "request_consumer";
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
            // let json = std::str::from_utf8(&delivery.data).unwrap();
            // println!("request data: {:?}", json);
            // channel
            //     .basic_reject(delivery.delivery_tag, BasicRejectOptions{ requeue: true })
            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await?
        }
    }
    Ok(())
}
