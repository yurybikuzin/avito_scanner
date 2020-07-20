
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
    future::{
        Fuse,
        FutureExt, // for `.fuse()`
    },
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

async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer_request = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;

    let mut consumer_proxies_to_use = {
        let queue_name = "proxies_to_use";
        let consumer_tag = "consumer_proxies_to_use";
        let _queue = get_queue(&channel, queue_name).await?;
        basic_consume(&channel, queue_name, consumer_tag).await?
    };

    println!("{} connected, waiting for messages", consumer_tag.as_ref());
    let mut fut_queue = FuturesUnordered::new();
    let mut reqs: VecDeque<(Req, amq_protocol_types::LongLongUInt)> = VecDeque::new();
    let mut proxy_urls: VecDeque<(String, usize, amq_protocol_types::LongLongUInt)> = VecDeque::new();
    let mut proxy_hosts: HashSet<String> = HashSet::new();
    loop {
        // Пока есть необработанные запросы (reqs) и свободные проки (proxy_urls)
        // формируем пары и заполняем fut_queue
        while reqs.len() > 0 && proxy_urls.len() > 0 {
            let (url, success_count, delivery_tag_proxy) = proxy_urls.pop_front().unwrap();
            let (req, delivery_tag_req) = reqs.pop_front().unwrap();
            let client = reqwest::Client::builder()
                .proxy(reqwest::Proxy::all(&url).unwrap())
                .timeout(
                    req.timeout.unwrap_or(
                        Duration::from_secs(RESPONSE_TIMEOUT.load(Ordering::Relaxed) as u64)
                    )
                )
                .build()?
            ;
            trace!("took {} for {}", url, req.url);
            fut_queue.push(op(OpArg::Fetch(fetch::Arg {
                client, 
                opt: fetch::Opt {
                    req,
                    delivery_tag_req,
                    url: url.to_owned(),
                    success_count,
                    delivery_tag_proxy
                },
            })));
        }

        // Если есть вакансии на обработку запросов (fut_queue.len() < SAME_TIME_REQUEST_MAX)
        // то принимает запросы из раббита
        let consumer_request_next_fut = if fut_queue.len() < SAME_TIME_REQUEST_MAX.load(Ordering::Relaxed) as usize {
            consumer_request.next().fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_request_next_fut);

        // Если свободных прокси не хватает для поступивших запросов
        // то отправляем команду "load_list" (загрузить списки прокси на проверку)
        // и принимаем проверенные прокси из раббита
        let consumer_proxies_to_use_next_fut = if proxy_urls.len() < reqs.len() {
            // super::cmd::publish_load_list(&channel).await?;
            consumer_proxies_to_use.next().fuse()
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_proxies_to_use_next_fut);

        let consumer_timeout_fut = if proxy_hosts.len() < PROXIES_TO_USE_MIN_COUNT.load(Ordering::Relaxed) as usize {
            // super::cmd::publish_load_list(&channel).await?;
            if STATE_PROXIES_TO_USE.load(Ordering::Relaxed) == STATE_PROXIES_TO_USE_FILLED {
                trace!("tokio::time::delay_for due to STATE_PROXIES_TO_USE_FILLED");
                tokio::time::delay_for(std::time::Duration::from_secs(5)).fuse()
            } else {
                Fuse::terminated()
            }
        } else {
            Fuse::terminated()
        };
        pin_mut!(consumer_timeout_fut);

        // Если свободных прокси не хватает для поступивших запросов и поднят флаг "очередь прокси
        // заполняется" заводим будильник на 5 секунд, чтобы через 5 секунд 
        // проверить, не опустился ли флаr (тогда отправим команду "load_list" или придут
        // проверенные прокси )
        // let consumer_timeout_fut = if proxy_urls.len() < reqs.len() && STATE_PROXIES_TO_USE.load(Ordering::Relaxed) == STATE_PROXIES_TO_USE_FILLED {
        //     trace!("tokio::time::delay_for due to STATE_PROXIES_TO_USE_FILLED");
        //     tokio::time::delay_for(std::time::Duration::from_secs(5)).fuse()
        // } else {
        //     Fuse::terminated()
        // };
        // pin_mut!(consumer_timeout_fut);

        select! {
            // прозвенел будильник, сбрасываем флаг STATE_PROXIES_TO_USE до STATE_PROXIES_TO_USE_NONE
            ret = consumer_timeout_fut => {
                STATE_PROXIES_TO_USE.store(STATE_PROXIES_TO_USE_NONE, Ordering::Relaxed);
            },
            // пришел проверенный прокси
            consumer_proxies_to_use_next_opt = consumer_proxies_to_use_next_fut => {
                if let Some(consumer_proxies_to_use_next) = consumer_proxies_to_use_next_opt {
                    if let Ok((channel, delivery)) = consumer_proxies_to_use_next {
                        let url = std::str::from_utf8(&delivery.data).unwrap();
                        // проверяем, что он валидный (мало ли кто там чего в очередь раббита
                        // напихал)
                        if let Ok(_url_proxy) = reqwest::Proxy::all(url) {
                            // проверяем, что такого url'а с таким же хостом у нас нет среди уже используемых
                            // le
                            if let Some(host) = get_host_of_url(url)  {
                                if !proxy_hosts.contains(&host) {
                                    // И только валидный url с уникальным хостом помещаем в конец
                                    // очереди доступных прокси,
                                    // устанавливая кредит доверия ему в размере SUCCESS_COUNT_START
                                    proxy_urls.push_back(
                                        (url.to_owned(), 
                                         SUCCESS_COUNT_START.load(Ordering::Relaxed) as usize, delivery.delivery_tag),
                                    );
                                    // добавляем host этого url'а в наше множество уникальных
                                    // хостов
                                    proxy_hosts.insert(host);
                                } else {
                                    // если url содержит не уникальный хост, 
                                    // то просто убираем этот url из очереди раббита в небытие
                                    basic_ack(&channel, delivery.delivery_tag).await?;
                                    trace!("url {} has non uniq host {}", url, host);
                                }
                            } else {
                                // если не удалось извлечь хост из url'а
                                // то просто убираем этот url из очереди раббита в небытие
                                basic_ack(&channel, delivery.delivery_tag).await?;
                                error!("could not get_host_of_url: {}", url);
                            }
                        } else {
                            // если url не валидный
                            // то просто убираем этот url из очереди раббита в небытие
                            basic_ack(&channel, delivery.delivery_tag).await?;
                            error!("invalid url: {}", url);
                        }
                    } else {
                        error!("Err(err) = consumer_proxies_to_use_next");
                    }
                } else {
                    error!("consumer_proxies_to_use_next_opt.is_none()");
                }
            },
            // пришел запрос на обработку
            consumer_request_next_opt = consumer_request_next_fut => {
                if let Some(consumer_request_next) = consumer_request_next_opt {
                    if let Ok((channel, delivery)) = consumer_request_next {
                        let s = std::str::from_utf8(&delivery.data).unwrap();
                        let req: Req = serde_json::from_str(&s).unwrap();
                        // помещаем его в конец очереди запросов на обработку
                        reqs.push_back((req, delivery.delivery_tag));
                    } else {
                        error!("Err(err) = consumer_request_next");
                    }
                } else {
                    error!("consumer_request_next_opt.is_none()");
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
                            OpRet::ReuseProxy(ret) => {
                                let reuse_proxy::Ret { url, success_count, delivery_tag, push_front } = ret;
                                // если предыдущая попытка использовния прокси была успешной, 
                                // то помещаем url в начало очереди
                                if push_front {
                                    info!("{}(success: {}) to be reused", url, success_count);
                                    proxy_urls.push_front((url, success_count, delivery_tag));
                                } else {
                                // если предыдущая попытка использовния прокси была не успешной, 
                                // то помещаем url в конец очереди
                                    debug!("{}(success: {}) to be reused", url, success_count);
                                    proxy_urls.push_back((url, success_count, delivery_tag));
                                }
                            },
                            // результаты обрабтки пары (запрос, прокси)
                            OpRet::Fetch(ret) => {
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
                                            fut_queue.push(op(OpArg::ReuseProxy(reuse_proxy::Arg {
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
                                                proxy_hosts.remove(&host);
                                            }
                                            let _queue = get_queue(&channel, queue_name).await?;
                                            let proxy_error = ProxyError{url, msg};
                                            let payload = serde_json::to_string_pretty(&proxy_error)?;
                                            basic_publish(&channel, queue_name, payload).await?;
                                            basic_ack(&channel, delivery_tag_proxy).await?;
                                        }

                                        reqs.push_front((req, delivery_tag_req));
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

                                                fut_queue.push(op(OpArg::ReuseProxy(reuse_proxy::Arg {
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
                                                        proxy_hosts.remove(&host);
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
                                                    fut_queue.push(op(OpArg::ReuseProxy(reuse_proxy::Arg {
                                                        url,
                                                        success_count: success_count - 1,
                                                        delivery_tag: delivery_tag_proxy,
                                                        push_front: false,
                                                    })));
                                                }
                                                // необработанный запрос возвращаем в начало
                                                // очереди запросов на обработки
                                                reqs.push_front((req, delivery_tag_req));
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
    }
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

enum OpArg {
    Fetch(fetch::Arg),
    ReuseProxy(reuse_proxy::Arg),
}

enum OpRet {
    Fetch(fetch::Ret),
    ReuseProxy(reuse_proxy::Ret),
}

async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::Fetch(arg) => {
            let ret = fetch::run(arg).await?;
            Ok(OpRet::Fetch(ret))
        },
        OpArg::ReuseProxy(arg) => {
            let ret = reuse_proxy::run(arg).await?;
            Ok(OpRet::ReuseProxy(ret))
        },
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

    // const COUNT_LIMIT: u64 = 4900;
    // const PRICE_PRECISION: isize = 20000;
    // const PRICE_MAX_INC: isize = 1000000;
    //
    // use term::Term;

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
        // let client_provider = client::Provider::new(client::Kind::Reqwest(1));
        // helper(client_provider).await?;

        // let pool = rmq::get_pool();
        // let client_provider = client::Provider::new(client::Kind::ViaProxy(pool));
        // helper(client_provider).await?;

        Ok(())
    }

    // async fn helper(client_provider: client::Provider) -> Result<()> {
    //
    //     let count_limit: u64 = env::get("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
    //     let price_precision: isize = env::get("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
    //     let price_max_inc: isize = env::get("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;
    //
    //
    //     let params = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
    //     let arg = Arg { 
    //         params, 
    //         count_limit, 
    //         price_precision, 
    //         price_max_inc,
    //         client_provider,
    //     };
    //
    //
    //     let mut term = Term::init(term::Arg::new().header("Определение диапазонов цен . . ."))?;
    //     let start = Instant::now();
    //     let mut auth = auth::Lazy::new(Some(auth::Arg::new()));
    //     let ret = get(&mut auth, arg, Some(|arg: CallbackArg| -> Result<()> {
    //         term.output(format!("count_total: {}, checks_total: {}, elapsed_millis: {}, per_millis: {}, detected_diaps: {}, price: {}..{}/{}, count: {}", 
    //             arg.count_total,
    //             arg.checks_total,
    //             arrange_millis::get(arg.elapsed_millis),
    //             arrange_millis::get(arg.per_millis),
    //             arg.diaps_detected,
    //             if arg.price_min.is_none() { "".to_owned() } else { arg.price_min.unwrap().to_string() },
    //             if arg.price_max.is_none() { "".to_owned() } else { arg.price_max.unwrap().to_string() },
    //             if arg.price_max_delta.is_none() { "".to_owned() } else { arg.price_max_delta.unwrap().to_string() },
    //             if arg.count.is_none() { "".to_owned() } else { arg.count.unwrap().to_string() },
    //         ))
    //     })).await?;
    //     println!("{}, Определены диапазоны цен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.diaps.len());
    //
    //     info!("ret:{}", serde_json::to_string_pretty(&ret).unwrap());
    //
    //     Ok(())
    // }
}

