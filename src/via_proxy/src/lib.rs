
#![recursion_limit="2048"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;

use futures::{
    StreamExt,
    future::{
        Fuse, 
        FutureExt,// for `.fuse()`
    }, 
    select,
    pin_mut,
};

use std::sync::{Mutex, Arc};
use lazy_static::lazy_static;
use std::collections::HashMap;
use futures::executor::ThreadPool;
use promise::Promise;

// ============================================================================
// ============================================================================


type BrokerRef = Arc<Broker>;
#[derive(Clone)]
pub struct Client {
    broker: BrokerRef,
}

impl Drop for Client {
    fn drop(&mut self) {
        trace!("drop client");
        if Arc::strong_count(&self.broker) <= 2 {
            let mut brokers = BROKERS.lock().unwrap();
            brokers.remove(&self.broker.name);
            futures::executor::block_on(self.send_stop());
            debug!("strong_count: {}", Arc::strong_count(&self.broker));
        }
    }
}

lazy_static! {
    static ref BROKERS: Mutex<HashMap<String, BrokerRef>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
enum Msg {
    Stop(Promise<()>),
    Req(Req),
}

#[derive(Debug)]
struct Req {
    url: Url,
    promise: Promise<Res>,
}

#[derive(Debug, Clone)]
struct Res {
    status: http::StatusCode,
    text: String,
}

use tokio::sync::mpsc;
impl Client {
    pub async fn new(pool: rmq::Pool) -> Result<Self> {
        Self::new_special(pool, "response".to_owned()).await
    }
    pub async fn new_special(pool: rmq::Pool, reply_to: String) -> Result<Self> {
        let mut brokers = BROKERS.lock().unwrap();
        let broker = match brokers.get(reply_to.as_str()) {
            Some(broker) => broker.clone(),
            None => {
                let (sender, mut receiver): (mpsc::Sender<Msg>, mpsc::Receiver<Msg>) = mpsc::channel(10000);
                let broker_name = reply_to.to_owned();
                let broker = Arc::new(Broker {
                    name: broker_name.to_owned(),
                    sender: Mutex::new(sender),
                });
                let future = async move { 
                    let future = async move { 
                        trace!("in future {:?}", reply_to);

                        let conn = rmq::get_conn(pool).await?;
                        let channel = conn.create_channel().await?;
                        let queue_name: &str = reply_to.as_ref();
                        let _queue = rmq::get_queue(&channel, queue_name).await?;
                        let consumer_tag = format!("{}_consumer", reply_to);
                        let mut consumer = rmq::basic_consume(&channel, queue_name, consumer_tag).await?;

                        let mut reqs: HashMap<uuid::Uuid, Promise<Res>> = HashMap::new();
                        let mut stop_promise = None;
                        loop {
                            let consumer_next_fut = if stop_promise.is_some() {
                                Fuse::terminated()
                            } else {
                                consumer.next().fuse()
                            };
                            let receiver_next_fut = if stop_promise.is_some() {
                                Fuse::terminated()
                            } else {
                                receiver.next().fuse()
                            };
                            pin_mut!(consumer_next_fut);
                            pin_mut!(receiver_next_fut);
                            select! {
                                receiver_next_opt = receiver_next_fut => {
                                    trace!("received: {:?}", receiver_next_opt);
                                    if let Some(receiver_next) = receiver_next_opt {
                                        match receiver_next {
                                            Msg::Stop(promise) => {
                                                stop_promise = Some(promise);
                                            },
                                            Msg::Req (Req { url, mut promise }) => {
                                                let correlation_id = uuid::Uuid::new_v4();
                                                reqs.insert(correlation_id, promise);
                                                let req = req::Req {
                                                    correlation_id,
                                                    reply_to: queue_name.to_owned(),
                                                    method: http::Method::GET,
                                                    url,
                                                    timeout: None,
                                                    no_proxy: None,
                                                };
                                                let payload = serde_json::to_string_pretty(&req)?;
                                                trace!("before basic_publish");
                                                rmq::basic_publish(&channel, "request", payload).await?;
                                                trace!("after basic_publish");
                                            },
                                        }
                                    } else {
                                        error!("receiver_next_opt.is_none()");
                                    }
                                },
                                consumer_next_opt = consumer_next_fut => {
                                    trace!("consumed: {:?}", consumer_next_opt);
                                    if let Some(consumer_next) = consumer_next_opt {
                                        if let Ok((channel, delivery)) = consumer_next {
                                            let s  = std::str::from_utf8(&delivery.data)?;
                                            let res: res::Res = serde_json::from_str(&s).context("via_proxy::Client::get_text()")?;
                                            rmq::basic_ack(&channel, delivery.delivery_tag).await?;
                                            if let Some(mut promise) = reqs.remove(&res.correlation_id) {
                                                promise.resolve(Res { status: res.status, text: res.text });
                                            }
                                        } else {
                                            error!("Err(err) = consumer_next");
                                        }
                                    } else {
                                        error!("consumer_next_opt.is_none()");
                                    }
                                },
                                complete => {
                                    trace!("complete");
                                    break;
                                },
                            }
                        }
                        if let Some(mut stop_promise) = stop_promise {
                            stop_promise.resolve(());
                        }
                        trace!("out future {:?}", queue_name);
                        Ok::<(), Error>(())
                    };
                    if let Err(err) = future.await {
                        error!("future: {}", err);
                    }
                };
                let pool = ThreadPool::new().unwrap();
                pool.spawn_ok(future);
                brokers.insert(broker_name, broker.clone());
                broker
            },
        };
        Ok(Self {
            broker,
        })
    }
    pub async fn send_stop(&mut self) {
        let promise: Promise<()> = Promise::new();
        &self.broker.sender.lock().unwrap().send(Msg::Stop(promise.clone())).await.unwrap();
        promise.await;
    }
    pub async fn get_text_status(&self, url: Url) -> Result<(String, http::StatusCode)> {
        let promise: Promise<Res> = Promise::new();
        let req = Req { url, promise: promise.clone() };
        &self.broker.sender.lock().unwrap().send(Msg::Req(req)).await?;
        let res = promise.await;
        return Ok((res.text, res.status))
    }
}

struct Broker {
    sender: Mutex<mpsc::Sender<Msg>>,
    name: String,
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| pretty_env_logger::init());
    }

    use futures::{
        select,
        stream::{
            FuturesUnordered,
        },
    };

    async fn op(client: Client, url: Url) -> Result<(Url, String)> {
        let fut = client.get_text_status(url.clone());
        let (text, _status) = fut.await?;
        Ok((url, text))
    }

    #[tokio::test]
    async fn test_via_proxy() -> Result<()> {
        init();

        let pool = rmq::get_pool();
        {
            let client = Client::new(pool.clone()).await?;
            let mut fut_queue = FuturesUnordered::new();

            let url = Url::parse("https://bikuzin.baza-winner.ru/echo?some")?;
            fut_queue.push(op(client.clone(), url));
            let url = Url::parse("https://bikuzin.baza-winner.ru/echo?thing")?;
            fut_queue.push(op(client.clone(), url));
            let url = Url::parse("https://bikuzin.baza-winner.ru/echo?good")?;
            fut_queue.push(op(client.clone(), url));
            loop {
                select! {
                    ret = fut_queue.select_next_some() => {
                        match ret {
                            Err(err) => {
                                bail!("error: {}", err);
                            },
                            Ok((url, text)) => {
                                info!("url: {}, text: {}", url, text);
                            },
                        }
                    }
                    complete => {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

}
