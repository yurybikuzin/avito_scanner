
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::collections::VecDeque;
use std::time::Duration;
use futures::{StreamExt};
use rmq::{
    get_conn, 
    get_queue, 
    Pool, 
    basic_consume, 
    basic_publish, 
    basic_ack,
};

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "request";
        let consumer_tag = "request_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_tag),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_tag, e),
        };
    }
}

use req::Req;
use res::Res;

use futures::{
    channel::mpsc::{self, Receiver, Sender},
    select,
    pin_mut,
    stream::{
        FuturesUnordered,
    },
};

use serde::Serialize;
#[derive(Serialize)]
struct ProxyError {
    url: String,
    msg: String,
}

use super::*;
use std::collections::HashSet;

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

macro_rules! fut_fetch {
    ($fut_queue: expr, $req: expr, $url: expr, $proxy: expr, $success_count: expr, $delivery_tag_req: expr, $delivery_tag_proxy: expr) => {
        let client = reqwest::Client::builder()
            .proxy($proxy)
            .timeout(
                $req.timeout.unwrap_or(
                    Duration::from_secs(RESPONSE_TIMEOUT.load(Ordering::Relaxed) as u64)
                )
            )
            .build()?
        ;
        trace!("took {} for {}", $url, $req.url);
        $fut_queue.push(op::run(op::Arg::Fetch(fetch::Arg {
            client, 
            opt: fetch::Opt {
                req: $req,
                delivery_tag_req: $delivery_tag_req,
                url: $url.to_owned(),
                success_count: $success_count,
                delivery_tag_proxy: $delivery_tag_proxy,
            },
        })));
    };
}

macro_rules! fut_fetch_if_free_proxy_exists {
    ($fut_queue: expr, $proxy_url_queue: expr, $req: expr, $delivery_tag_req: expr, $else: block) => {
        // если есть свободный прокси, выполняем запрос через этот прокси
        if let Some(ProxyUrlQueueItem {url, success_count, delivery_tag: delivery_tag_proxy}) = $proxy_url_queue.pop_front() {
            fut_fetch!($fut_queue, $req, url, reqwest::Proxy::all(&url).unwrap(), success_count, $delivery_tag_req, delivery_tag_proxy);
        } else {
            $else
        }
    };
}

macro_rules! fut_fetch_if_non_processed_request_exists {
    ($fut_queue: expr, $req_queue: expr, $url: expr, $proxy: block, $success_count: expr, $delivery_tag_proxy: expr, $else: block) => {
        // если есть необработанный запрос, выполняем этот запрос через прокси
        if let Some(ReqQueueItem {req, delivery_tag: delivery_tag_req}) = $req_queue.pop_front() {
            let proxy = $proxy;
            fut_fetch!($fut_queue, req, $url, proxy, $success_count, delivery_tag_req, $delivery_tag_proxy);
        } else {
            $else
        }
    };
}


struct ReqQueueItem {
    req: Req,
    delivery_tag: amq_protocol_types::LongLongUInt,
}

struct ProxyUrlQueueItem {
    url: String,
    success_count: usize,
    delivery_tag: amq_protocol_types::LongLongUInt,
}

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        error!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;

    let thread_pool = futures::executor::ThreadPool::new().unwrap();

    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;
    get_receiver!(receiver_request, consumer, "request", thread_pool);

    let mut consumer = {
        let queue_name = "proxies_to_use";
        let consumer_tag = "consumer_proxies_to_use";
        let _queue = get_queue(&channel, queue_name).await?;
        basic_consume(&channel, queue_name, consumer_tag).await?
    };
    get_receiver!(receiver_proxies_to_use, consumer, "proxies_to_use", thread_pool);

    let mut req_queue: VecDeque<ReqQueueItem> = VecDeque::new();
    let mut proxy_url_queue: VecDeque<ProxyUrlQueueItem> = VecDeque::new();
    let mut fut_queue = FuturesUnordered::new();
    let mut proxy_host_set: HashSet<String> = HashSet::new();

    let receiver_request_fut = receiver_request.next();
    pin_mut!(receiver_request_fut);
    let receiver_proxies_to_use_fut = receiver_proxies_to_use.next();
    pin_mut!(receiver_proxies_to_use_fut);
    loop {
        select! {
            // пришел запрос на обработку
            ret = receiver_request_fut => {
                if let Some((s, delivery_tag)) = ret {
                    let req: Req = serde_json::from_str(&s).unwrap();
                    // если есть свободный прокси, выполняем запрос через этот прокси
                    fut_fetch_if_free_proxy_exists!(fut_queue, proxy_url_queue, req, delivery_tag, {
                    // в противном случае помещаем его в конец очереди запросов на обработку
                        req_queue.push_back(ReqQueueItem{req, delivery_tag});
                    });

                }
            },
            // пришел проверенный прокси
            ret = receiver_proxies_to_use_fut => {
                if let Some((url, delivery_tag)) = ret {
                    // проверяем, что он валидный (мало ли кто там чего в очередь раббита
                    // напихал)
                    if let Ok(proxy) = reqwest::Proxy::all(&url) {
                        // проверяем, что такого url'а с таким же хостом у нас нет среди уже используемых
                        // le
                        if let Some(host) = get_host_of_url(&url)  {
                            if !proxy_host_set.contains(&host) {
                                // И только для валидного url'а с уникальным хостом проверяем
                                // если есть необработанный запрос, выполняем этот запрос через прокси
                                // в противном случае помещаем в конец очереди доступных прокси,
                                let success_count = SUCCESS_COUNT_START.load(Ordering::Relaxed) as usize;
                                fut_fetch_if_non_processed_request_exists!(fut_queue, req_queue, url, { proxy }, success_count, delivery_tag, {
                                    // в противном случае помещаем в конец очереди доступных прокси,
                                    // устанавливая кредит доверия ему в размере SUCCESS_COUNT_START
                                    proxy_url_queue.push_back(ProxyUrlQueueItem {
                                        url, 
                                        success_count, 
                                        delivery_tag,
                                    });
                                    // добавляем host этого url'а в наше множество уникальных
                                    // хостов
                                    proxy_host_set.insert(host);
                                });
                            } else {
                                // если url содержит не уникальный хост, 
                                // то просто убираем этот url из очереди раббита в небытие
                                basic_ack(&channel, delivery_tag).await?;
                                trace!("url {} has non uniq host {}", url, host);
                            }
                        } else {
                            // если не удалось извлечь хост из url'а
                            // то просто убираем этот url из очереди раббита в небытие
                            basic_ack(&channel, delivery_tag).await?;
                            warn!("could not get_host_of_url: {}", url);
                        }
                    } else {
                        // если url не валидный
                        // то просто убираем этот url из очереди раббита в небытие
                        basic_ack(&channel, delivery_tag).await?;
                        warn!("invalid url: {}", url);
                    }
                }
            },
            // результаты обрабтки пары (запрос, прокси)
            // а также отложенные просьбы вернуть освободившийся прокси в очередь достпуных прокси
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(_) => {
                        error!("unreachable at request.rs:224");
                        unreachable!();
                    },
                    Ok(ret) => {
                        match ret {
                            // отложенные просьбы вернуть освободившийся прокси в очередь достпуных прокси
                            op::Ret::ReuseProxy(ret) => {
                                let reuse_proxy::Ret { url, success_count, delivery_tag, push_front } = ret;
                                fut_fetch_if_non_processed_request_exists!(fut_queue, req_queue, url, { reqwest::Proxy::all(&url).unwrap() }, success_count, delivery_tag, {
                                    // если предыдущая попытка использовния прокси была успешной, 
                                    // то помещаем url в начало очереди
                                    if push_front {
                                        info!("{}(success: {}) to be reused", url, success_count);
                                        proxy_url_queue.push_front(ProxyUrlQueueItem {url, success_count, delivery_tag});
                                    } else {
                                    // если предыдущая попытка использовния прокси была не успешной, 
                                    // то помещаем url в конец очереди
                                        debug!("{}(success: {}) to be reused", url, success_count);
                                        proxy_url_queue.push_back(ProxyUrlQueueItem {url, success_count, delivery_tag});
                                    }
                                });
                            },
                            // результаты обрабтки пары (запрос, прокси)
                            op::Ret::Fetch(ret) => {
                                let fetch::Ret{ret, opt} = ret;
                                let fetch::Opt { req, delivery_tag_req, url, success_count, delivery_tag_proxy } = opt;
                                match ret {
                                    // обработка пары (запрос, прокси) завершилась ошибкой
                                    fetch::RequestRet::Err(err) => {
                                        // выясняем природу ошибки
                                        let (queue_name, msg) =  if err.is_timeout() {
                                            ("proxies_error_timeout", format!("is_timeout: {}", err))
                                        } else if err.is_builder() {
                                            ("proxies_error_builder", format!("is_builder({}): {}", url, err))
                                        } else if err.is_status() {
                                            ("proxies_error_status", format!("is_status({}): {}, status: {:?}", url, err, err.status()))
                                        } else if err.is_redirect() {
                                            ("proxies_error_redirect", format!("is_redirect({}): {}, url: {:?}", url, err, err.url()))
                                        } else {
                                            ("proxies_error_other", format!("other({}): {}", url, err))
                                        };

                                        // если эта ошибка timeout и при этом кредит доверия этому
                                        // прокси еще не исчерпан
                                        if err.is_timeout() && success_count > 0 {
                                            debug!("{}: {}", url, msg);
                                            // то формируем отложенную просьбу вернуть прокси в
                                            // конец очереди доступных прокси с уменьшиме кредита
                                            // доверия
                                            fut_queue.push(op::run(op::Arg::ReuseProxy(reuse_proxy::Arg {
                                                url,
                                                success_count: success_count - 1,
                                                delivery_tag: delivery_tag_proxy,
                                                push_front: false,
                                            })));
                                        } else {
                                            // если ошибка не timeout или кредит доверия к прокси
                                            // исчерпан, то переносим прокси из одной очереди раббита (proxies_to_use) в другую (proxies_timeout)
                                            // и удаляем хост этого url'а из нашего множества
                                            // уникальных хостов
                                            warn!("{}: {}", url, msg);
                                            if let Some(host) = get_host_of_url(&url) {
                                                proxy_host_set.remove(&host);
                                            }
                                            let _queue = get_queue(&channel, queue_name).await?;
                                            let proxy_error = ProxyError{url, msg};
                                            let payload = serde_json::to_string_pretty(&proxy_error)?;
                                            basic_publish(&channel, queue_name, payload).await?;
                                            basic_ack(&channel, delivery_tag_proxy).await?;
                                        }

                                        // если есть свободный прокси, выполняем запрос через этот прокси
                                        fut_fetch_if_free_proxy_exists!(fut_queue, proxy_url_queue, req, delivery_tag_req, {
                                        // в противном случае помещаем его в начало очереди запросов на обработку
                                            req_queue.push_front(ReqQueueItem {req, delivery_tag: delivery_tag_req});
                                        });
                                    },
                                    // обаботка пары (запрос, прокси) как-то завершилась (без
                                    // ошибки)
                                    fetch::RequestRet::Ok{url: url_res, text, status} => {
                                        // анализируем статус полученного ответа
                                        match status {
                                            // в этом случае к прокси претензий нет
                                            // поэтому формируем отложенный запрос на возвращение
                                            // прокси в очередь доступных прокси с увеличением
                                            // кредита доверия
                                            // ответ на запрос помещаем в очередь ответов раббита
                                            // удаляя при этом запрос из очереди запросов раббита
                                            // как обработанный
                                            http::StatusCode::OK | http::StatusCode::BAD_REQUEST | http::StatusCode::NOT_FOUND => {
                                                info!("{}: {}: {}", url, req.url, status);
                                                let queue_name: &str = req.reply_to.as_ref();
                                                let _queue = get_queue(&channel, queue_name).await?;
                                                let res = Res {
                                                    correlation_id: req.correlation_id,
                                                    url_req: req.url.clone(),
                                                    url_res,
                                                    status,
                                                    text,
                                                };
                                                basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                                basic_ack(&channel, delivery_tag_req).await?;

                                                fut_queue.push(op::run(op::Arg::ReuseProxy(reuse_proxy::Arg {
                                                    url,
                                                    success_count: if success_count >= SUCCESS_COUNT_MAX.load(Ordering::Relaxed) as usize { success_count } else { success_count + 1 },
                                                    delivery_tag: delivery_tag_proxy,
                                                    push_front: true,
                                                })));
                                            },
                                            // а в этом случае есть вопросы к прокси
                                            _ => {
                                                warn!("{}(success: {}): {:?}", url, success_count, status);
                                                // если кредит доверия к прокси исчерпан
                                                if success_count == 0 {
                                                    // то перемещаем прокси из одной очереди
                                                    // раббита (proxies_to_use) в другую
                                                    // (proxies_status_СТАТУС)
                                                    // и удаляем хост этого прокси из нашего
                                                    // множества уникальных хостов
                                                    if let Some(host) = get_host_of_url(&url) {
                                                        proxy_host_set.remove(&host);
                                                    }
                                                    let queue_name = format!("proxies_status_{:?}", status);
                                                    let _queue = get_queue(&channel, queue_name.as_str(),).await?;
                                                    basic_publish(&channel, queue_name.as_str(), url).await?;
                                                    basic_ack(&channel, delivery_tag_proxy).await?;
                                                } else {
                                                    // а вот если кредит доверия к прокси еще не
                                                    // исчерпан, то формируем отложеннй запрос на
                                                    // возвращение прокси в конец очереди доступных
                                                    // прокси с уменьшением кредита доверия
                                                    fut_queue.push(op::run(op::Arg::ReuseProxy(reuse_proxy::Arg {
                                                        url,
                                                        success_count: success_count - 1,
                                                        delivery_tag: delivery_tag_proxy,
                                                        push_front: false,
                                                    })));
                                                }
                                                // если есть свободный прокси, выполняем запрос через этот прокси
                                                fut_fetch_if_free_proxy_exists!(fut_queue, proxy_url_queue, req, delivery_tag_req, {
                                                // в противном случае помещаем его в начало очереди запросов на обработку
                                                    req_queue.push_front(ReqQueueItem {req, delivery_tag: delivery_tag_req});
                                                });
                                            },
                                        }
                                    }
                                }
                            },
                        }
                    },
                }
            },
            complete => {
                error!("complete: unreachable!");
                break;
            },
        }
    };
    Ok(())
}

use lazy_static::lazy_static;
use regex::Regex;

fn get_host_of_url(url: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
            ^
            (?P<proto>.*?)
            ://
            (?P<host>.*)
            $
        "#).unwrap();
    }
    match RE.captures(url) {
        Some(caps) => Some(caps.name("host").unwrap().as_str().to_owned()),
        None => None,
    }
}

mod op {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use super::*;

    pub enum Arg {
        Fetch(fetch::Arg),
        ReuseProxy(reuse_proxy::Arg),
    }

    pub enum Ret {
        Fetch(fetch::Ret),
        ReuseProxy(reuse_proxy::Ret),
    }

    pub async fn run(arg: Arg) -> Result<Ret> {
        match arg {
            Arg::Fetch(arg) => {
                let ret = fetch::run(arg).await?;
                Ok(Ret::Fetch(ret))
            },
            Arg::ReuseProxy(arg) => {
                let ret = reuse_proxy::run(arg).await?;
                Ok(Ret::ReuseProxy(ret))
            },
        }
    }
}

mod reuse_proxy {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use super::super::*;

    pub struct Arg {
        pub url: String,
        pub success_count: usize,
        pub delivery_tag: amq_protocol_types::LongLongUInt,
        pub push_front: bool,
    }

    pub type Ret = Arg;

    use std::time::Duration;
    pub async fn run(arg: Arg) -> Result<Ret> {
        tokio::time::delay_for(Duration::from_millis(PROXY_REST_DURATION.load(Ordering::Relaxed) as u64)).await;
        Ok(arg)
    }
}

mod fetch {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use amq_protocol_types::LongLongUInt;

    use super::{Req};

    pub struct Arg {
        pub client: reqwest::Client,
        pub opt: Opt,
    }

    pub struct Ret {
        pub ret: RequestRet,
        pub opt: Opt,
    }

    pub enum RequestRet {
        Err(reqwest::Error),
        Ok{
            url: reqwest::Url,
            status: http::StatusCode,
            text: String,
        },
    }

    pub struct Opt {
        pub req: Req,
        pub delivery_tag_req: LongLongUInt,
        pub url: String,
        pub success_count: usize,
        pub delivery_tag_proxy: LongLongUInt,
    }

    pub async fn run(arg: Arg) -> Result<Ret> {
        let Arg {  client, opt } = arg;
        match client.request(opt.req.method.clone(), opt.req.url.clone()).send().await {
            Err(err) => Ok(Ret { 
                ret: RequestRet::Err(err),
                opt,
            }),
            Ok(response) => {
                let url = response.url().clone();
                let status = response.status();
                match response.text().await {
                    Err(err) => Ok(Ret { 
                        ret: RequestRet::Err(err),
                        opt,
                    }),
                    Ok(text) => Ok(Ret {
                        ret: RequestRet::Ok{
                            url,
                            status,
                            text,
                        },
                        opt,
                    })
                }
            }
        }
    }
}


// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;

    // docker exec -it -e RUST_LOG=diaps=trace -e AVITO_AUTH=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir avito-proj cargo test -p diaps -- --nocapture
    #[tokio::test]
    async fn test_get_host_of_proxy_url() -> Result<()> {
        test_helper::init();

        let url = "http://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        let url = "socks5://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        let url = "https://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        Ok(())
    }

}

