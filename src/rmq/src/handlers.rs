#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use super::req::Req;
use warp::{Rejection, Reply};
use super::rmq::{self, Pool};
use lapin::{ Queue, Channel };
type WebResult<T> = std::result::Result<T, Rejection>;
use super::error::{self};

impl warp::reject::Reject for error::Error {}

pub async fn health() -> WebResult<impl Reply> {
    Ok("OK")
}

pub async fn req(req: Req, pool: Pool) -> WebResult<impl Reply> {
    info!("got: {}", serde_json::to_string_pretty(&req).unwrap());
    let payload = serde_json::to_string_pretty(&req).unwrap();
    let rmq_conn = rmq::get_conn(pool).await.map_err(|e| {
        eprintln!("can't connect to rmq, {}", e);
        warp::reject::custom(error::Error::RMQPoolError(e))
    })?;

    let channel = rmq_conn.create_channel().await.map_err(|e| {
        eprintln!("create_channel: {}", e);
        warp::reject::custom(error::Error::RMQError(Error::new(e)))
    })?;

    let queue = get_queue(&channel, "proxies_to_use").await?;
    if queue.message_count() == 0 {
        let queue = get_queue(&channel, "proxies_to_check").await?;
        if queue.message_count() == 0 {
            let queue = get_queue(&channel, "fetch_proxies").await?;
            if queue.message_count() == 0 {
                basic_publish(&channel, "fetch_proxies", "").await?;
            }
        }
    }
    basic_publish(&channel, "request", payload).await?;
    Ok("OK")
}

async fn basic_publish<S: AsRef<str>, S2: AsRef<str>>(channel: &Channel, queue_name: S, payload: S2) -> WebResult<()>  {
    rmq::basic_publish(channel, queue_name.as_ref(), payload.as_ref()).await
        .map_err(|e| {
            eprintln!("can't publish: {}", e);
            warp::reject::custom(error::Error::RMQError(e))
        })?;
    Ok(())
}

async fn get_queue<S: AsRef<str>>(channel: &Channel, queue_name: S) -> WebResult<Queue> {
    rmq::get_queue(channel, queue_name.as_ref()).await
        .map_err(|e| {
            eprintln!("queue_declare'{}': {}", queue_name.as_ref(), e);
            warp::reject::custom(error::Error::RMQError(e))
        })
}

