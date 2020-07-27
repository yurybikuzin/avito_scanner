
#![recursion_limit="2048"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;

use futures::{
    StreamExt,
    channel::mpsc::{self, Receiver, Sender},
    // future::{
    //     // Fuse, 
    //     // FutureExt,// for `.fuse()`
    // }, 
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

macro_rules! get_receiver {
    ($receiver: ident, $consumer: expr, $kind: expr, $thread_pool: expr) => {
        let (mut sender, mut $receiver): (Sender<(String, u64)>, Receiver<(String, u64)>) = mpsc::channel(1000);
        $thread_pool.spawn_ok(async move {
            while let Some(consumer_next) = $consumer.next().await {
                if let Ok((_channel, delivery)) = consumer_next {
                    let s = std::str::from_utf8(&delivery.data).unwrap();
                    sender.try_send((s.to_owned(), delivery.delivery_tag)).unwrap();
                } else {
                    error!("{}: Err(err) = consumer_next", $kind);
                }
            }
            error!("{}: consumer stopped", $kind);
        });
    };
}

// use tokio::sync::mpsc;
impl Client {
    pub async fn new(pool: rmq::Pool, queue_name: String) -> Result<Self> {
        Self::new_special(pool, queue_name).await
    }
    pub async fn new_special(pool: rmq::Pool, reply_to: String) -> Result<Self> {
        let mut brokers = BROKERS.lock().unwrap();
        let broker = match brokers.get(reply_to.as_str()) {
            Some(broker) => broker.clone(),
            None => {
                let (sender, mut receiver_request): (mpsc::Sender<Msg>, mpsc::Receiver<Msg>) = mpsc::channel(10000);
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

                        let thread_pool = ThreadPool::new().unwrap();
                        get_receiver!(receiver_response, consumer, "response", thread_pool);

                        let receiver_response_fut = receiver_response.next();
                        pin_mut!(receiver_response_fut);

                        let mut reqs: HashMap<uuid::Uuid, Promise<Res>> = HashMap::new();
                        // let mut stop_promise = None;
                        let receiver_request_fut = receiver_request.next();
                        pin_mut!(receiver_request_fut);
                        loop {
                            // let consumer_next_fut = if stop_promise.is_some() {
                            //     Fuse::terminated()
                            // } else {
                            //     consumer.next().fuse()
                            // };
                            // let receiver_request_fut = if stop_promise.is_some() {
                            //     Fuse::terminated()
                            // } else {
                            //     receiver_request.next().fuse()
                            // };
                            // pin_mut!(consumer_next_fut);
                            // pin_mut!(receiver_request_fut);
                            select! {
                                receiver_next_opt = receiver_request_fut => {
                                    if let Some(receiver_next) = receiver_next_opt {
                                        match receiver_next {
                                            Msg::Stop(mut stop_promise) => {
                                                trace!("received stop signal");
                                                // stop_promise = Some(promise);
                                                stop_promise.resolve(());
                                                break;
                                            },
                                            Msg::Req (Req { url, mut promise }) => {
                                                let correlation_id = uuid::Uuid::new_v4();
                                                trace!("received request, made correlation_id: {:?}", correlation_id);
                                                reqs.insert(correlation_id, promise);
                                                let req = req::Req {
                                                    correlation_id,
                                                    reply_to: queue_name.to_owned(),
                                                    method: http::Method::GET,
                                                    url,
                                                    timeout: None,
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
                                ret = receiver_response_fut => {
                                    // if let Some(consumer_next) = consumer_next_opt {
                                    //     if let Ok((channel, delivery)) = consumer_next {
                                        if let Some((s, delivery_tag)) = ret {
                                            trace!("consumed: {:?}", delivery_tag);
                                            // let s  = std::str::from_utf8(&delivery.data)?;
                                            let res: res::Res = serde_json::from_str(&s).context("via_proxy::Client::get_text()")?;
                                            rmq::basic_ack(&channel, delivery_tag).await?;
                                            if let Some(mut promise) = reqs.remove(&res.correlation_id) {
                                                promise.resolve(Res { status: res.status, text: res.text });
                                                trace!("promise resolved for correlation_id: {}", res.correlation_id);
                                            } else {
                                                warn!("no promise for correlation_id {}", res.correlation_id);
                                            }
                                        } else {
                                            error!("Err(err) = consumer_next");
                                        }
                                    // } else {
                                    //     error!("consumer_next_opt.is_none()");
                                    // }
                                },
                                complete => {
                                    trace!("complete");
                                    break;
                                },
                            }
                        }
                        // if let Some(mut stop_promise) = stop_promise {
                        //     stop_promise.resolve(());
                        // }
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
        // &self.broker.sender.lock().unwrap().send(Msg::Stop(promise.clone())).await.unwrap();
        &self.broker.sender.lock().unwrap().try_send(Msg::Stop(promise.clone())).unwrap();
        promise.await;
    }
    pub async fn get_text_status(&self, url: Url) -> Result<(String, http::StatusCode)> {
        let promise: Promise<Res> = Promise::new();
        let req = Req { url: url.clone(), promise: promise.clone() };
        // &self.broker.sender.lock().unwrap().send(Msg::Req(req)).await?;
        &self.broker.sender.lock().unwrap().try_send(Msg::Req(req)).unwrap();
        trace!("get_text_status is waiting for promise of {}", url);
        let res = promise.await;
        trace!("resolved get_text_status for {}", url);
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
