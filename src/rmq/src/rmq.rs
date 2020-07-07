#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use lapin::{
    Queue, Channel, Consumer,
    options::{QueueDeclareOptions, BasicConsumeOptions, BasicPublishOptions}, 
    types::FieldTable, 
    ConnectionProperties,
    BasicProperties,
};
use tokio_amqp::*;
use deadpool_lapin::{Manager
    , PoolError
};

pub type Pool = deadpool::managed::Pool<lapin::Connection, lapin::Error>;
pub type Connection = deadpool::managed::Object<lapin::Connection, lapin::Error>;
pub type RMQResult<T> = std::result::Result<T, PoolError>;

pub fn get_pool() -> Pool {
    let addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://rmq:rmq@rmq:5672/%2f".into());
    let manager = Manager::new(addr, ConnectionProperties::default().with_tokio());
    deadpool::managed::Pool::new(manager, 10)
}

pub async fn get_conn(pool: Pool) -> RMQResult<Connection> {
    let conn = pool.get().await?;
    Ok(conn)
}

pub async fn get_queue<S: AsRef<str>>(channel: &Channel, queue_name: S) -> Result<Queue> {
    let queue = channel
        .queue_declare(
            queue_name.as_ref(),
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
    // println!("Declared queue {:?}", queue);
    Ok(queue)
}


pub async fn basic_consume<S: AsRef<str>, S2: AsRef<str>>(channel: &Channel, queue_name: S, consumer_tag: S2) -> Result<Consumer> {
    let consumer = channel
        .basic_consume(
            queue_name.as_ref(),
            consumer_tag.as_ref(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    Ok(consumer)
}

pub async fn basic_publish<S: AsRef<str>, S2: AsRef<str>>(channel: &Channel, queue_name: S, payload: S2) -> Result<()>  {
    channel
        .basic_publish(
            "",
            queue_name.as_ref(),
            BasicPublishOptions::default(),
            payload.as_ref().as_bytes().to_vec(),
            BasicProperties::default(),
        )
        .await?
        // .map_err(|e| {
        //     eprintln!("can't publish: {}", e);
        //     warp::reject::custom(error::Error::RMQError(Error::new(e)))
        // })?
        // .await?
    ;
        // .map_err(|e| {
        //     eprintln!("can't publish: {}", e);
        //     warp::reject::custom(error::Error::RMQError(Error::new(e)))
        // })?;
    Ok(())
}

