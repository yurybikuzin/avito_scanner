#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
// #[allow(unused_imports)]
// use anyhow::{Result, Error, bail, anyhow};

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use lapin::{
    Queue, Channel, Consumer,
    options::{
        QueueDeclareOptions, 
        BasicConsumeOptions, 
        BasicPublishOptions, 
        BasicAckOptions, 
        // BasicRejectOptions,
    }, 
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

mod settings;
pub use settings::{Settings};
pub fn get_pool(settings: Settings) -> Result<Pool> {
    // let user = std::env::var("BW_RABBITMQ_USER").context("BW_RABBITMQ_USER")?;
    // let pass = std::env::var("BW_RABBITMQ_PASS").context("BW_RABBITMQ_PASS")?;
    // let host = std::env::var("BW_RABBITMQ_HOST").context("BW_RABBITMQ_HOST")?;
    // let port = std::env::var("BW_RABBITMQ_PORT").context("BW_RABBITMQ_PORT")?;
    // let vhost = std::env::var("BW_RABBITMQ_VHOST").context("BW_RABBITMQ_VHOST")?;
    // let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://rmq:rmq@rmq:5672/%2f".into());
    // let addr = format!("amqp://{}:{}@{}:{}/{}", user, pass, host, port, vhost);
    // info!("addr: {}", addr);
    let manager = Manager::new(settings.addr(), ConnectionProperties::default().with_tokio());
    Ok(deadpool::managed::Pool::new(manager, 10))
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
    ;
    Ok(())
}

pub async fn basic_ack(channel: &Channel, delivery_tag: amq_protocol_types::LongLongUInt) -> Result<()> {
    channel.basic_ack(delivery_tag, BasicAckOptions::default()).await?;
    Ok(())
}

// pub async fn basic_reject(channel: &Channel, delivery_tag: amq_protocol_types::LongLongUInt) -> Result<()> {
//     channel.basic_reject(delivery_tag, BasicRejectOptions{ requeue: true }).await?;
//     Ok(())
// }


