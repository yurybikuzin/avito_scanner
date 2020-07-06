#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
// #[allow(unused_imports)]
// use anyhow::{Result, Error, bail, anyhow};

use deadpool_lapin::{Manager, PoolError};
use futures::{join, StreamExt};
use lapin::{options::*, types::FieldTable, BasicProperties, ConnectionProperties};
use std::convert::Infallible;
use std::result::Result as StdResult;
use std::time::Duration;
use thiserror::Error as ThisError;
use tokio_amqp::*;
use warp::{Filter, Rejection, Reply};
//
type WebResult<T> = StdResult<T, Rejection>;
type RMQResult<T> = StdResult<T, PoolError>;
type Result<T> = StdResult<T, Error>;
//
type Pool = deadpool::managed::Pool<lapin::Connection, lapin::Error>;
type Connection = deadpool::managed::Object<lapin::Connection, lapin::Error>;

#[derive(ThisError, Debug)]
enum Error {
    #[error("rmq error: {0}")]
    RMQError(#[from] lapin::Error),
    #[error("rmq pool error: {0}")]
    RMQPoolError(#[from] PoolError),
}
//
// impl warp::reject::Reject for Error {}
// // TODO: use latest master in deadpool-lapin
//
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let addr =
        // std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://rmq:rmq@127.0.0.1:5672/%2f".into());
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://rmq:rmq@rmq:5672/%2f".into());
    let manager = Manager::new(addr, ConnectionProperties::default().with_tokio());
    let pool = deadpool::managed::Pool::new(manager, 10);

    let health_route = warp::path!("health").and_then(health_handler);
    let add_req_route = warp::path!("req")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(with_rmq(pool.clone()))
        .and_then(add_req_handler)
    ;
    let routes = health_route.or(add_req_route);

    println!("Started server at localhost:8000");
    let _ = join!(
        warp::serve(routes).run(([0, 0, 0, 0], 8000)),
        rmq_listen(pool.clone())
    );
    Ok(())
}
//
// // mod api {
// //
// //     // use super::handlers;
// //     // use super::models::{Db, ListOptions, Todo};
// //     use warp::Filter;
// //
// //     /// The 4 TODOs filters combined.
// //     pub fn api(
// //         db: Db,
// //     ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
// //         warp::path!("todos")
// //             .and(warp::get())
// //             .and(warp::query::<ListOptions>())
// //             .and(with_db(db))
// //             .and_then(handlers::list_todos)
// //     }
// // }
//
fn with_rmq(pool: Pool) -> impl Filter<Extract = (Pool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}
//
// use serde::{
//     // Serialize, 
//     Deserialize};
use serde::ser::{Serialize, Serializer, SerializeStruct};
//
// // #[derive(Debug, Serialize, Deserialize)]
// // #[serde(deny_unknown_fields)]
struct Req {
    method: reqwest::Method,
    url: reqwest::Url,
}

impl Serialize for Req {
    // fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 2 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("Req", 2)?;
        let method = match self.method {
            reqwest::Method::GET => "GET",
            reqwest::Method::POST => "POST",
            reqwest::Method::PUT => "PUT",
            reqwest::Method::DELETE => "DELETE",
            reqwest::Method::HEAD => "HEAD",
            reqwest::Method::OPTIONS => "OPTIONS",
            reqwest::Method::CONNECT => "CONNECT",
            reqwest::Method::PATCH => "PATCH",
            reqwest::Method::TRACE => "TRACE",
            _ => unreachable!(),
        };
        state.serialize_field("method", &method)?;
        state.serialize_field("url", &self.url.as_str())?;
        state.end()
    }
}

// use serde::{};
use std::fmt;

use serde::de::{self, 
     Deserialize, 
    Deserializer, Visitor, 
    // SeqAccess, 
    MapAccess};

impl<'de> Deserialize<'de> for Req {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // This part could also be generated independently by:
        //
       // #[derive(Deserialize)]
       // #[serde(field_identifier, rename_all = "lowercase")]
       enum Field { Method, Url }
        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> StdResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`secs` or `nanos`")
                    }

                    fn visit_str<E>(self, value: &str) -> StdResult<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "method" => Ok(Field::Method),
                            "url" => Ok(Field::Url),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ReqVisitor;

        impl<'de> Visitor<'de> for ReqVisitor {
            type Value = Req;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Req")
            }

            // fn visit_seq<V>(self, mut seq: V) -> StdResult<Req, V::Error>
            // where
            //     V: SeqAccess<'de>,
            // {
            //     let method: String = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(0, &self))?;
            //     le
            //     let url = seq.next_element()?
            //         .ok_or_else(|| de::Error::invalid_length(1, &self))?;
            //     Ok(Req {method, url})
            // }

            fn visit_map<V>(self, mut map: V) -> StdResult<Req, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut method = None;
                let mut url = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Method => {
                            if method.is_some() {
                                return Err(de::Error::duplicate_field("method"));
                            }
                            let s: &str = map.next_value()?;
                            method = Some(match s {
                                "GET" => reqwest::Method::GET,
                                "POST" => reqwest::Method::POST,
                                "PUT" => reqwest::Method::PUT,
                                "DELETE" => reqwest::Method::DELETE,
                                "HEAD" => reqwest::Method::HEAD,
                                "OPTIONS" => reqwest::Method::OPTIONS,
                                "CONNECT" => reqwest::Method::CONNECT,
                                "PATCH" => reqwest::Method::PATCH,
                                "TRACE" => reqwest::Method::TRACE,
                                _ => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s), &"valid HTTP method")),
                            });
                        }
                        Field::Url => {
                            if url.is_some() {
                                return Err(de::Error::duplicate_field("url"));
                            }
                            let s: &str = map.next_value()?;
                            url = Some(match reqwest::Url::parse(s) {
                                Err(_) => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s), &"valid url")),
                                Ok(url) => url,
                            });
                        }
                    }
                }
                let method = method.ok_or_else(|| de::Error::missing_field("method"))?;
                let url = url.ok_or_else(|| de::Error::missing_field("url"))?;
                Ok(Req {method, url})
            }
        }

        const FIELDS: &'static [&'static str] = &["secs", "nanos"];
        deserializer.deserialize_struct("Req", FIELDS, ReqVisitor)
    }
}

async fn add_req_handler(req: Req, pool: Pool) -> WebResult<impl Reply> {
    // info!("req: {:?}", req);
    // let payload = b"Hello world!";
    let payload = serde_json::to_string_pretty(&req).unwrap();
    let payload = payload.as_bytes();

    let rmq_con = get_rmq_con(pool).await.map_err(|e| {
        eprintln!("can't connect to rmq, {}", e);
        warp::reject::custom(Error::RMQPoolError(e))
    })?;

    let channel = rmq_con.create_channel().await.map_err(|e| {
        eprintln!("can't create channel, {}", e);
        warp::reject::custom(Error::RMQError(e))
    })?;

    channel
        .basic_publish(
            "",
            "request",
            BasicPublishOptions::default(),
            payload.to_vec(),
            BasicProperties::default(),
        )
        .await
        .map_err(|e| {
            eprintln!("can't publish: {}", e);
            warp::reject::custom(Error::RMQError(e))
        })?
        .await
        .map_err(|e| {
            eprintln!("can't publish: {}", e);
            warp::reject::custom(Error::RMQError(e))
        })?;
    Ok("OK")
}

async fn health_handler() -> WebResult<impl Reply> {
    Ok("OK")
}

async fn get_rmq_con(pool: Pool) -> RMQResult<Connection> {
    let connection = pool.get().await?;
    Ok(connection)
}

async fn rmq_listen(pool: Pool) -> Result<()> {
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
            "request",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
    println!("Declared queue {:?}", queue);

    let mut consumer = channel
        .basic_consume(
            "request",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("rmq consumer connected, waiting for messages");
    while let Some(delivery) = consumer.next().await {
        if let Ok((channel, delivery)) = delivery {
            println!("received request: {:?}", delivery);
            let json = std::str::from_utf8(&delivery.data).unwrap();
            println!("request data: {:?}", json);
// delivery.some();
            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await?
        }
    }
    Ok(())
}
