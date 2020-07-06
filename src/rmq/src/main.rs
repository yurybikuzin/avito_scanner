#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use futures::{join };


type Pool = deadpool::managed::Pool<lapin::Connection, lapin::Error>;
type Connection = deadpool::managed::Object<lapin::Connection, lapin::Error>;

mod error {
    use thiserror::Error as ThisError;
    use deadpool_lapin::PoolError;
    #[derive(ThisError, Debug)]
    pub enum Error {
        #[error("rmq error: {0}")]
        RMQError(#[from] lapin::Error),
        #[error("rmq pool error: {0}")]
        RMQPoolError(#[from] PoolError),
    }
}

impl warp::reject::Reject for error::Error {}
// TODO: use latest master in deadpool-lapin
//
use std::env;
#[tokio::main]
async fn main() -> Result<()> {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=todos=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "rmq=info");
    }
    pretty_env_logger::init();

    let pool = rmq::pool();

    let routes = api::api(pool.clone());

    println!("Started server at localhost:8000");
    let _ = join!(
        warp::serve(routes).run(([0, 0, 0, 0], 8000)),
        rmq::rmq_listen(pool.clone())
    );
    Ok(())
}

mod api {
    use super::Pool;
    use warp::Filter;
    use super::handlers;
    use std::convert::Infallible;
    use warp::{Rejection, Reply};

    pub fn api(
        pool: Pool,
    ) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        health_route()
            .or(add_msg_route(pool))
    }

    /// GET /health
    pub fn health_route() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::path!("health").and_then(handlers::health)
    }

    /// POST /req
    pub fn add_msg_route(pool: Pool) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
        warp::path!("req")
            .and(warp::post())
            .and(with_rmq(pool.clone()))
            .and_then(handlers::req)
    }

    fn with_rmq(pool: Pool) -> impl Filter<Extract = (Pool,), Error = Infallible> + Clone {
        warp::any().map(move || pool.clone())
    }

}

mod handlers {
    use warp::{Rejection, Reply};
    use super::{rmq, error, Pool};
    use lapin::{
        options::BasicPublishOptions, 
        // types::FieldTable, 
        BasicProperties, 
        // ConnectionProperties,
    };
    type WebResult<T> = std::result::Result<T, Rejection>;

    pub async fn health() -> WebResult<impl Reply> {
        Ok("OK")
    }

    pub async fn req(pool: Pool) -> WebResult<impl Reply> {
        let payload = b"Hello world!";

        let rmq_con = rmq::get_rmq_con(pool).await.map_err(|e| {
            eprintln!("can't connect to rmq, {}", e);
            warp::reject::custom(error::Error::RMQPoolError(e))
        })?;

        let channel = rmq_con.create_channel().await.map_err(|e| {
            eprintln!("can't create channel, {}", e);
            warp::reject::custom(error::Error::RMQError(e))
        })?;

        channel
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                eprintln!("can't publish: {}", e);
                warp::reject::custom(error::Error::RMQError(e))
            })?
            .await
            .map_err(|e| {
                eprintln!("can't publish: {}", e);
                warp::reject::custom(error::Error::RMQError(e))
            })?;
        Ok("OK")
    }

}

mod rmq {
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use super::{Pool, Connection, 
        // RMQResult
    };
    use lapin::{
        options::{QueueDeclareOptions, BasicConsumeOptions, BasicAckOptions}, 
        types::FieldTable, 
        // BasicProperties, 
        ConnectionProperties,
    };
    use std::time::Duration;
    use futures::{StreamExt};
    use tokio_amqp::*;
    use deadpool_lapin::{Manager, PoolError};

    type RMQResult<T> = std::result::Result<T, PoolError>;

    pub fn pool() -> Pool {
        let addr =
            std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://rmq:rmq@rmq:5672/%2f".into());
        let manager = Manager::new(addr, ConnectionProperties::default().with_tokio());
        deadpool::managed::Pool::new(manager, 10)
    }

    pub async fn get_rmq_con(pool: Pool) -> RMQResult<Connection> {
        let connection = pool.get().await?;
        Ok(connection)
    }

    pub async fn rmq_listen(pool: Pool) -> Result<()> {
        let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            retry_interval.tick().await;
            println!("connecting rmq consumer...");
            match init_rmq_listen(pool.clone()).await {
                Ok(_) => println!("rmq listen returned"),
                Err(e) => eprintln!("rmq listen had an error: {}", e),
            };
        }
    }

    async fn init_rmq_listen(pool: Pool) -> Result<()> {
        let rmq_con = get_rmq_con(pool).await.map_err(|e| {
            eprintln!("could not get rmq con: {}", e);
            e
        })?;
        let channel = rmq_con.create_channel().await?;

        let queue = channel
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        println!("Declared queue {:?}", queue);

        let mut consumer = channel
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        println!("rmq consumer connected, waiting for messages");
        while let Some(delivery) = consumer.next().await {
            if let Ok((channel, delivery)) = delivery {
                println!("received msg: {:?}", delivery);
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await?
            }
        }
        Ok(())
    }
}


