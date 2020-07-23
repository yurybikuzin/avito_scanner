
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
    ($fut_queue: expr, $fetch_req_count: ident, $req: expr, $url: expr, $proxy: expr, $success_count: expr, $delivery_tag_req: expr) => {
        let client = reqwest::Client::builder()
            .proxy($proxy)
            .timeout(
                $req.timeout.unwrap_or(
                    Duration::from_secs(RESPONSE_TIMEOUT.load(Ordering::Relaxed) as u64)
                )
            )
            .build()?
        ;
        // trace!("took {} for {}", $url, $req.url);
        $fetch_req_count += 1;
        info!("Отправляем запрос '{}' на обработку через прокси '{}', в обработке теперь: {}", $delivery_tag_req, $url, $fetch_req_count);
        $fut_queue.push(op::run(op::Arg::FetchReq(fetch_req::Arg {
            client, 
            opt: fetch_req::Opt {
                req: $req,
                delivery_tag_req: $delivery_tag_req,
                url: $url.to_owned(),
                success_count: $success_count,
            },
        })));
    };
}

macro_rules! start_proxy_check {
    ($fut_queue: expr, $source_fetch_count: ident, $proxy_url_queue_to_check: expr, $proxy_url_check_count: expr, $sources: expr, $source_queue_to_fetch: expr) => {
        if $source_queue_to_fetch.len() == 0 && $source_fetch_count == 0 && $proxy_url_queue_to_check.len() == 0 && $proxy_url_check_count == 0 {
            // начинаем со чтения списка url'ов источников
            for source in $sources.iter() {
                $source_queue_to_fetch.push_back(source);
            }
            info!("Заполнили очередь источников прокси: {}", $source_queue_to_fetch.len());
            // пока есть вакантные места на fetch источников, заполняем их
            while $source_fetch_count < SAME_TIME_REQUEST_MAX.load(Ordering::Relaxed) { 
                if let Some(source) = $source_queue_to_fetch.pop_front() {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
                        .build()?
                    ;
                    $fut_queue.push(op::run(op::Arg::FetchSource(fetch_source::Arg{client, source: source.to_owned()})));

                    $source_fetch_count += 1;
                } else {
                    break;
                }
            }
            info!("Запустили одновременное чтение источников прокси: {}", $source_fetch_count);
        } else if $source_queue_to_fetch.len() != 0 {
            info!("Сейчас очередь источников прокси: {}", $source_queue_to_fetch.len());
        } else if $source_fetch_count != 0 {
            info!("Сейчас fetch'ится источников прокси: {}", $source_fetch_count);
        } else if $proxy_url_queue_to_check.len() != 0 {
            info!("Сейчас очередь прокси на проверку: {}", $proxy_url_queue_to_check.len());
        } else {
            info!("Сейчас проверяется прокси: {}", $proxy_url_check_count);
        }
    };
}


macro_rules! fut_fetch_if_free_proxy_exists {
    ($fut_queue: expr, $fetch_req_count: ident, $reuse_proxy_count: expr, $proxy_url_queue: expr, $req: expr, $delivery_tag_req: expr, $source_fetch_count: ident, $proxy_url_queue_to_check: expr, $proxy_url_check_count: expr, $sources: expr, $source_queue_to_fetch: expr, $else: block) => {
                // если есть свободный прокси, выполняем запрос через этот прокси
        if let Some(ProxyUrlQueueItem {url, success_count, latency: _}) = $proxy_url_queue.pop_front() {
            info!("Есть свободный прокси, осталось еще: {}, еще {} используются и {} в режиме отложенной просьбы на возврат в очередь свободных прокси", $proxy_url_queue.len(), $fetch_req_count, $reuse_proxy_count);
            fut_fetch!($fut_queue, $fetch_req_count, $req, url, reqwest::Proxy::all(&url).unwrap(), success_count, $delivery_tag_req);
        } else {
            warn!("Свободных прокси нет, но на проверке еще прокси: {}, и еще ждут своей очереди: {}", $proxy_url_check_count, $proxy_url_queue_to_check.len());
            $else
            // поскольку список свободных прокси пуст, запускаем процесс дополнения
            // списк свободных прокси, но предварительно убедившись, что он уже не запущен
            start_proxy_check!($fut_queue, $source_fetch_count, $proxy_url_queue_to_check, $proxy_url_check_count, $sources, $source_queue_to_fetch);
        }
    };
}

macro_rules! fut_fetch_if_free_proxy_exists_or_push_front {
    ($fut_queue: expr, $fetch_req_count: ident, $reuse_proxy_count: expr, $proxy_url_queue: expr, $req: expr, $delivery_tag_req: expr, $source_fetch_count: ident, $proxy_url_queue_to_check: expr, $proxy_url_check_count: expr, $sources: expr, $source_queue_to_fetch: expr, $req_queue: expr) => {
        // если есть свободный прокси, выполняем запрос через этот прокси
        fut_fetch_if_free_proxy_exists!($fut_queue, $fetch_req_count, $reuse_proxy_count, $proxy_url_queue, $req, $delivery_tag_req, $source_fetch_count, $proxy_url_queue_to_check, $proxy_url_check_count, $sources, $source_queue_to_fetch, {
        // в противном случае помещаем его в начало очереди запросов на обработку
            $req_queue.push_front(ReqQueueItem {req: $req, delivery_tag: $delivery_tag_req});
            info!("поместили запрос '{}' в начало очереди запросов на обработку, теперь в очереди: {}", $delivery_tag_req, $req_queue.len());
        });
    };
}

macro_rules! fut_fetch_if_non_processed_request_exists {
    ($fut_queue: expr, $fetch_req_count: ident, $req_queue: expr, $url: expr, $proxy: block, $success_count: expr, $else: block) => {
        // если есть необработанный запрос, выполняем этот запрос через прокси
        if let Some(ReqQueueItem {req, delivery_tag: delivery_tag_req}) = $req_queue.pop_front() {
            info!("Есть необработанный запрос '{}', осталось еще: {}, и еще {} в обработке", delivery_tag_req, $req_queue.len(), $fetch_req_count);
            let proxy = $proxy;
            fut_fetch!($fut_queue, $fetch_req_count, req, $url, proxy, $success_count, delivery_tag_req);
        } else {
            warn!("Необработанных запросов нет{}",
                if $fetch_req_count == 0 {
                    "".to_owned()
                } else {
                    format!(", но в обработке еще запросов: {}", $fetch_req_count)
                }
            );
            $else
        }
    };
}

macro_rules! fut_push_check_proxy {
    ($fut_queue: expr, $url: expr, $own_ip_opt: ident) => {
        let own_ip = match $own_ip_opt.take() {
            None => match get_own_ip().await {
                Ok(own_ip) => own_ip,
                Err(err) => {
                    bail!("FATAL: failed to get own_ip: {}", err);
                },
            },
            Some(own_ip) => {
                if Instant::now().duration_since(own_ip.last_update).as_secs() < OWN_IP_FRESH_DURATION.load(Ordering::Relaxed) as u64 {
                    own_ip
                } else {
                    match get_own_ip().await {
                        Ok(own_ip) => own_ip,
                        Err(err) => {
                            warn!("old own_ip {} will be used due to {}", own_ip.ip, err);
                            own_ip
                        },
                    }
                }
            },
        };
        let proxy = match reqwest::Proxy::all(&$url) {
            Ok(proxy) => proxy,
            Err(err) => {
                bail!("malformed url {}: {}", $url, err);
            },
        };
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
            .build()?
        ;
        let opt = check_proxy::Opt { url: $url };
        $fut_queue.push(op::run(op::Arg::CheckProxy(check_proxy::Arg { 
            client, 
            own_ip: own_ip.ip.to_owned(), 
            opt,
        })));
        $own_ip_opt = Some(own_ip);
    };
}

macro_rules! fut_reuse_proxy {
    ($fut_queue: expr, $reuse_proxy_count: ident, $url: expr, $latency: expr, $success_count: expr) => {
        $fut_queue.push(op::run(op::Arg::ReuseProxy(reuse_proxy::Arg {
            url: $url,
            success_count: $success_count,
            latency: $latency,
        })));
        $reuse_proxy_count += 1;
    };
}

struct ReqQueueItem {
    req: Req,
    delivery_tag: amq_protocol_types::LongLongUInt,
}

struct ProxyUrlQueueItem {
    url: String,
    success_count: usize,
    latency: Option<u128>,
}

fn add_item_to_proxy_url_queue(item: ProxyUrlQueueItem, mut queue: VecDeque<ProxyUrlQueueItem>) -> VecDeque<ProxyUrlQueueItem> {
    match item.latency {
        None => {
            trace!("Помещаем в конец очереди url '{}', потому что его latency None", item.url);
            queue.push_back(item);
        },
        Some(latency) => {
            let mut i_found: Option<usize> = None;
            for (i, item) in queue.iter().enumerate() {
                match item.latency {
                    None => { 
                        i_found = Some(i);
                        break;
                    },
                    Some(latency_item) => {
                        if latency_item > latency {
                            i_found = Some(i);
                            break;
                        }
                    },
                }
            }
            match i_found {
                None => {
                    trace!("Помещаем в конец очереди url '{}', потому что его latency {} больше остальных {}", item.url, latency, queue.len());
                    queue.push_back(item);
                },
                Some(i) => {
                    trace!("Помещаем url '{}' с latency {} перед {}-м элементом из {}", item.url, latency, i, queue.len());
                    queue.insert(i, item);
                },
            }
        },
    }
    queue
}

use std::collections::HashMap;
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

    let mut req_queue: VecDeque<ReqQueueItem> = VecDeque::new();
    let mut proxy_url_queue: VecDeque<ProxyUrlQueueItem> = VecDeque::new();
    let mut fut_queue = FuturesUnordered::new();
    let mut proxy_host_set: HashSet<String> = HashSet::new();

    let receiver_request_fut = receiver_request.next();
    pin_mut!(receiver_request_fut);

    let sources: Vec<&str> = vec![
        "https://raw.githubusercontent.com/fate0/proxylist/master/proxy.list", // 394 => 179
        "https://raw.githubusercontent.com/a2u/free-proxy-list/master/free-proxy-list.txt", // 4 => 1
        "http://rootjazz.com/proxies/proxies.txt", // 925 => 88
        "https://socks-proxy.net/", // 300 => 3
        "http://online-proxy.ru/", // 1769 => 19,
    ];
    let protos = vec!["socks5", "http", "https"];
    let mut proxy_url_queue_to_check = VecDeque::<String>::new();
    let mut proxy_url_check_count = 0usize;
    // let proxy_url_check_count_same_time_max = 50usize;
    let mut source_queue_to_fetch = VecDeque::<&str>::new();
    let mut source_fetch_count = 0usize;
    // let source_fetch_count_same_time_max = 10usize;
    let mut own_ip_opt: Option<OwnIp> = None;
    pub struct Checked {
        pub status: check_proxy::Status,
        pub url: String,
    }
    let mut checking = HashMap::<String, Vec<Checked>>::new();
    let mut fetch_req_count = 0usize;
    let mut reuse_proxy_count = 0usize;

    start_proxy_check!(fut_queue, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, sources, source_queue_to_fetch);
    loop {
        select! {
            // пришел запрос на обработку
            ret = receiver_request_fut => {
                if let Some((s, delivery_tag)) = ret {
                    info!("Пришел запрос {} на обработку", delivery_tag);
                    let req: Req = serde_json::from_str(&s).unwrap();
                    // если есть свободный прокси, выполняем запрос через этот прокси
                    fut_fetch_if_free_proxy_exists!(fut_queue, fetch_req_count, reuse_proxy_count, proxy_url_queue, req, delivery_tag, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, sources, source_queue_to_fetch, {
                        // в противном случае помещаем его в конец очереди запросов на обработку
                        req_queue.push_back(ReqQueueItem{req, delivery_tag});
                        trace!("Поместили запрос в конец очереди запросов на обработку: {}", req_queue.len());
                    });
                }
            },
            ret = fut_queue.select_next_some() => {
                match ret {
                    op::Ret::FetchSource(ret) => {
                        info!("Пришел результат fetch'а источника прокси '{}'", ret.arg.source);
                        match ret.result {
                            // могла произойти ошибка при этом
                            Err(err) => {
                                error!("fetch_source '{}': {}", ret.arg.source, err);
                            },
                            // в случае успеха мы получаем список хостов
                            Ok(hosts) => {
                                info!("Обнаружено хостов: {}", hosts.len());
                                for host in hosts.into_iter() {
                                    // отбираем только те хосты, которые нами не используются еще
                                    if !proxy_host_set.contains(&host) {
                                        for proto in protos.iter() {
                                            let url = format!("{}://{}", proto, host);
                                            match reqwest::Proxy::all(&url) {
                                                Ok(_) => {
                                                    proxy_url_queue_to_check.push_back(url);
                                                },
                                                Err(err) => {
                                                    bail!("FATAL: malformed url {}: {}", url, err);
                                                },
                                            };
                                        }
                                        proxy_host_set.insert(host);
                                    } else {
                                        trace!("host {} проигноирован, так как уже используется нами", host);
                                    }
                                }
                                info!("Очередь прокси на проверкy: {}, используемых хостов: {}", proxy_url_queue_to_check.len(), proxy_host_set.len());
                                while proxy_url_check_count < SAME_TIME_PROXY_CHECK_MAX.load(Ordering::Relaxed) {
                                    match proxy_url_queue_to_check.pop_front() {
                                        None => break,
                                        Some(url) => {
                                            fut_push_check_proxy!(fut_queue, url, own_ip_opt);
                                            proxy_url_check_count += 1;
                                        } 
                                    }
                                }
                                info!("Сейчас одновременно проверяется прокси: {}", proxy_url_check_count);
                            },
                        }
                        if let Some(source) = source_queue_to_fetch.pop_front() {
                            fut_queue.push(op::run(op::Arg::FetchSource(fetch_source::Arg{client: ret.arg.client, source: source.to_owned()})));
                        } else {
                            source_fetch_count -= 1;
                        }
                        info!("Сейчас fetch'ится источников: {}", source_fetch_count);
                    },
                    op::Ret::CheckProxy(check_proxy::Ret{status, opt}) => {
                        let check_proxy::Opt { url } = opt;
                        trace!("Пришел результат проверки прокси '{}': {:?}", url, 
                            match &status {
                                check_proxy::Status::Ok{ latency } => {
                                    format!("Ok, {}", arrange_millis::get(*latency))
                                },
                                check_proxy::Status::NonAnon => {
                                    "NonAnon".to_owned()
                                },
                                check_proxy::Status::Err(err) => {
                                    format!("Err: {}", err)
                                },
                            }
                        );
                        let host = get_host_of_url(&url)?;
                        let mut vec_checked = match checking.get_mut(&host) {
                            Some(vec_checked) => vec_checked,
                            None => {
                                let mut vec_checked = vec![];
                                checking.insert(host.clone(), vec_checked);
                                checking.get_mut(&host).unwrap()
                            },
                        };
                        vec_checked.push(Checked {
                            status,
                            url,
                        });
                        trace!("Для хоста '{}' из {} проверено {} прокси", host, protos.len(), vec_checked.len());
                        // Когда для каждого хоста накопится количество результатов, равное
                        // количеству протоколов (это значит мы проверили хост для каждого из
                        // протоколов)
                        if vec_checked.len() >= protos.len() {
                            // мы начнем выборы
                            struct Elected {
                                url: String,
                                latency: u128,
                            }
                            // Выбирать будем по принципу: живой (item.status == check_proxy::Status) и самый быстрый (min latency)
                            let mut elected: Option<Elected> = None;
                            for item in vec_checked.iter() {
                                match item.status {
                                    check_proxy::Status::Ok{latency} => {
                                        let url = item.url.clone();
                                        elected = match elected.take() {
                                            None => Some(Elected {url, latency}),
                                            Some(Elected{ url: url_elected, latency: latency_elected}) => {
                                                if latency_elected > latency {
                                                    Some(Elected{url, latency})
                                                } else {
                                                    Some(Elected{ url: url_elected, latency: latency_elected}) 
                                                }
                                            },
                                        };
                                        break;
                                    },
                                    _ => {},
                                }
                            }
                            match elected {
                                // Если выбрать не удалось (ни одного живого не обнаружил),
                                // то удаляем хост из числа используемых нами
                                None => {
                                    trace!("Для хоста '{}' выбирать не из чего: нет живых прокси", host);
                                    proxy_host_set.remove(&host);
                                },
                                Some(Elected {url, latency}) => {
                                // Если же удалось получить новый живой прокси
                                    info!("Для хоста '{}' выбран прокси '{}' с latency {}, осталось еще прокси для проверки: {}", host, url, latency, proxy_url_queue_to_check.len() + proxy_url_check_count - 1);
                                    let success_count = SUCCESS_COUNT_START.load(Ordering::Relaxed) as usize;
                                    // Проверяем очередь запросов, и если она не пуста, отправляем
                                    // запрос на выполнение с этим прокси
                                    fut_fetch_if_non_processed_request_exists!(fut_queue, fetch_req_count, req_queue, url, { reqwest::Proxy::all(&url).unwrap() }, success_count, {
                                        // в противном случае помещаем в очередь доступных прокси (согласно latency), устанавливая кредит доверия ему в размере SUCCESS_COUNT_START
                                        info!("Запросов, ожидающих обработки, нет{}, поэтому помещаем прокси '{}' согласно его latency {:?}, в очереди свободных прокси будет: {}", 
                                            if fetch_req_count == 0 { 
                                                "".to_owned() 
                                            } else { 
                                                format!(" (но в обработке еще запросов: {})", fetch_req_count)
                                            }, 
                                            url, 
                                            latency,
                                            proxy_url_queue.len() + 1, 
                                        );
                                        let item = ProxyUrlQueueItem {
                                            url, 
                                            success_count, 
                                            latency: Some(latency),
                                        };
                                        proxy_url_queue = add_item_to_proxy_url_queue(item, proxy_url_queue);
                                    });
                                },
                            }
                            // Удаляем хост из числа проверяемых
                            checking.remove(&host);
                            trace!("Сейчас в процессе проверки хостов: {}", checking.len());
                        }
                        if let Some(url) = proxy_url_queue_to_check.pop_front() {
                            trace!("Есть непроверенный прокси, осталось еще: {}", proxy_url_queue_to_check.len());
                            fut_push_check_proxy!(fut_queue, url, own_ip_opt);
                        } else {
                            proxy_url_check_count -= 1;
                        }
                        trace!("Сейчас одноврменно проверяется прокси: {}", proxy_url_check_count);
                    },
                    // отложенные просьбы вернуть освободившийся прокси в очередь достпуных прокси
                    op::Ret::ReuseProxy(ret) => {
                        reuse_proxy_count -= 1;
                        let reuse_proxy::Ret { url, success_count, latency } = ret;
                        info!("Пришла отложенная просьба вернуть в очередь прокси '{}' с latency {:?} и success_count {}, еще отложенных просьб: {}", url, latency, success_count, reuse_proxy_count);
                        // Проверяем очередь запросов, и если она не пуста, отправляем
                        // запрос на выполнение с этим прокси
                        fut_fetch_if_non_processed_request_exists!(fut_queue, fetch_req_count, req_queue, url, { reqwest::Proxy::all(&url).unwrap() }, success_count, {
                            // в противном случае помещаем в очередь доступных прокси (согласно latency)
                            info!("Запросов, ожидающих обработки, нет{}, поэтому помещаем прокси '{}' в очередь свободных прокси (длина до помещения: {}) согласно его latency {:?}", 
                                if fetch_req_count == 0 { 
                                    "".to_owned() 
                                } else { 
                                    format!(" (но в обработке еще запросов: {})", fetch_req_count)
                                }, 
                                url, 
                                proxy_url_queue.len(), 
                                latency,
                            );
                            let item = ProxyUrlQueueItem {url, success_count, latency};
                            proxy_url_queue = add_item_to_proxy_url_queue(item, proxy_url_queue);
                        });
                    },
                    // результаты обрабтки пары (запрос, прокси)
                    op::Ret::FetchReq(ret) => {
                        let fetch_req::Ret{result, opt} = ret;
                        let fetch_req::Opt { req, delivery_tag_req, url, success_count} = opt;
                        fetch_req_count -= 1;
                        info!("Пришел результат выполнения запроса {} через прокси '{}', еще в обработке запросов: {}, ждут своей очереди: {}", delivery_tag_req, url, fetch_req_count, req_queue.len());
                        match result {
                            // обработка пары (запрос, прокси) завершилась ошибкой
                            Err(err) => {
                                // выясняем природу ошибки
                                let msg =  if err.is_timeout() {
                                    format!("is_timeout: {}", err)
                                } else if err.is_builder() {
                                    format!("is_builder({}): {}", url, err)
                                } else if err.is_status() {
                                    format!("is_status({}): {}, status: {:?}", url, err, err.status())
                                } else if err.is_redirect() {
                                    format!("is_redirect({}): {}, url: {:?}", url, err, err.url())
                                } else {
                                    format!("other({}): {}", url, err)
                                };

                                // если эта ошибка timeout и при этом кредит доверия этому
                                // прокси еще не исчерпан
                                if err.is_timeout() && success_count > 0 {
                                    info!("он завершился по timeout, но кредит доверия {} к прокси '{}' не исчерпан, поэтому формируем отложенную просьбу на возврат его в очередь прокси с уменьшением кредита доверия и latency None (что гарантирует помещение в конец очереди)", success_count, url);
                                    fut_reuse_proxy!(fut_queue, reuse_proxy_count, url, None, success_count - 1);
                                } else {
                                    info!("он завершился ошибкой (кредит доверия при этом {}): {}", success_count, msg);
                                    // если ошибка не timeout или кредит доверия к прокси
                                    // и удаляем хост этого url'а из нашего множества уникальных хостов
                                    //
                                    let host = get_host_of_url(&url)?;
                                    proxy_host_set.remove(&host);
                                    info!("поэтому исключили его хост {} из числа используемых нами, осталось: {}", host, proxy_host_set.len());
                                }

                                // если есть свободный прокси, выполняем запрос через этот прокси, в противном случае помещаем его в начало очереди запросов на обработку
                                fut_fetch_if_free_proxy_exists_or_push_front!(fut_queue, fetch_req_count, reuse_proxy_count, proxy_url_queue, req, delivery_tag_req, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, sources, source_queue_to_fetch, req_queue);
                            },
                            // обаботка пары (запрос, прокси) как-то завершилась (без ошибки)
                            Ok(fetch_req::RetOk{url: url_res, text, status, latency}) => {
                                // анализируем статус полученного ответа
                                info!("он завершился со статусом {:?}", status);
                                match status {
                                    // в этом случае к прокси претензий нет
                                    // поэтому формируем отложенный запрос на возвращение
                                    // прокси в очередь доступных прокси с увеличением
                                    // кредита доверия
                                    // ответ на запрос помещаем в очередь ответов раббита
                                    // удаляя при этом запрос из очереди запросов раббита
                                    // как обработанный
                                    http::StatusCode::OK | http::StatusCode::BAD_REQUEST | http::StatusCode::NOT_FOUND => {
                                        let queue_name: &str = req.reply_to.as_ref();
                                        let _queue = get_queue(&channel, queue_name).await?;
                                        let res = Res {
                                            correlation_id: req.correlation_id,
                                            url_req: req.url.clone(),
                                            url_res,
                                            status,
                                            text,
                                        };
                                        info!("помещаем ответ на запрос {} в очередь ответов {}", delivery_tag_req, queue_name);
                                        basic_publish(&channel, queue_name, serde_json::to_string_pretty(&res).unwrap()).await?;

                                        info!("удаляем запрос {} из очереди запросов", delivery_tag_req);
                                        basic_ack(&channel, delivery_tag_req).await?;

                                        // let success_count = if success_count >= SUCCESS_COUNT_MAX.load(Ordering::Relaxed) as usize { success_count } else { success_count + 1 };
                                        info!("формируем отложенную просьбу на возврат прокси {} в очередь прокси с latency {} и увеличением кредита доверия до {}", url, latency, success_count);
                                        fut_reuse_proxy!(fut_queue, reuse_proxy_count, url, Some(latency), 
                                            if success_count >= SUCCESS_COUNT_MAX.load(Ordering::Relaxed) as usize { 
                                                    success_count 
                                                } else { 
                                                    success_count + 1 
                                            }
                                        );
                                    },
                                    http::StatusCode::FORBIDDEN  => {
                                        let host = get_host_of_url(&url)?;
                                        proxy_host_set.remove(&host);
                                        info!("прокси {} забанен, поэтому исключили его хост {} из числа используемых нами, осталось: {}", url, host, proxy_host_set.len());
                                        // если есть свободный прокси, выполняем запрос через этот прокси, в противном случае помещаем его в начало очереди запросов на обработку
                                        fut_fetch_if_free_proxy_exists_or_push_front!(fut_queue, fetch_req_count, reuse_proxy_count, proxy_url_queue, req, delivery_tag_req, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, sources, source_queue_to_fetch, req_queue);
                                    },
                                    // а в этом случае есть вопросы к прокси
                                    _ => {
                                        // если кредит доверия к прокси исчерпан
                                        if success_count == 0 {
                                            // то удаляем хост этого прокси из нашего
                                            // множества уникальных хостов
                                            let host = get_host_of_url(&url)?;
                                            proxy_host_set.remove(&host);
                                            info!("кредит доверия к прокси {} исчерпан, поэтому исключили его хост {} из числа используемых нами, осталось: {}", url, host, proxy_host_set.len());
                                        } else {
                                            // а вот если кредит доверия к прокси еще не
                                            // исчерпан, то формируем отложеннй запрос на
                                            // возвращение прокси в конец очереди доступных
                                            // прокси с уменьшением кредита доверия
                                            info!("поскольку кредит доверия {} к прокси '{}' не исчерпан, формируем отложенную просьбу на возврат его в очередь прокси с уменьшением кредита доверия и latency None (что гарантирует помещение в конец очереди)", success_count, url);
                                            fut_reuse_proxy!(fut_queue, reuse_proxy_count, url, None, success_count - 1);
                                        }
                                        // если есть свободный прокси, выполняем запрос через этот прокси, в противном случае помещаем его в начало очереди запросов на обработку
                                        fut_fetch_if_free_proxy_exists_or_push_front!(fut_queue, fetch_req_count, reuse_proxy_count, proxy_url_queue, req, delivery_tag_req, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, sources, source_queue_to_fetch, req_queue);
                                    },
                                }
                            }
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

mod check_proxy {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};

    pub struct Arg {
        pub client: reqwest::Client,
        pub own_ip: String,
        pub opt: Opt,
    }

    pub struct Ret {
        pub status: Status,
        pub opt: Opt,
    }

    pub struct Opt {
        pub url: String,
        // pub host: String,
    }

    pub enum Status {
        Ok{latency: u128},
        NonAnon,
        Err(Error),
    }

    pub async fn run(arg: Arg) -> Ret {
        let start = std::time::Instant::now();
        let status = match super::get_ip(arg.client).await {
            Err(err) => Status::Err(err),
            Ok(ip) => if ip != arg.own_ip {
                Status::Ok{ latency: std::time::Instant::now().duration_since(start).as_millis() }
            } else {
                Status::NonAnon
            },
        };
        Ret{
            status,
            opt: arg.opt,
        }
    }
}

use std::time::Instant;
pub struct OwnIp {
    ip: String,
    last_update: Instant,
}
pub async fn get_own_ip() -> Result<OwnIp> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
        .build()?
    ;
    let ip = get_ip( client).await?;
    Ok(OwnIp{
        ip, 
        last_update: Instant::now(),
    })
}

use json::{Json, By};
pub async fn get_ip(client: reqwest::Client) -> Result<String> {
    let url = "https://bikuzin18.baza-winner.ru/echo";
    let text = client.get(url)
        .send()
        .await?
        .text()
        .await?
    ;
    let json = Json::from_str(text, url)?;
    let ip = json.get([By::key("headers"), By::key("x-real-ip")])?.as_string()?;
    Ok(ip)
}

use lazy_static::lazy_static;
use regex::Regex;

fn get_host_of_url(url: &str) -> Result<String> {
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
        Some(caps) => Ok(caps.name("host").unwrap().as_str().to_owned()),
        None => Err(anyhow!("Failed to extract host from url: {}", url)),
    }
}

mod op {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use super::*;

    pub enum Arg {
        FetchSource(fetch_source::Arg),
        FetchReq(fetch_req::Arg),
        CheckProxy(check_proxy::Arg),
        ReuseProxy(reuse_proxy::Arg),
    }

    pub enum Ret {
        FetchSource(fetch_source::Ret),
        FetchReq(fetch_req::Ret),
        CheckProxy(check_proxy::Ret),
        ReuseProxy(reuse_proxy::Ret),
    }

    pub async fn run(arg: Arg) -> Ret {
        match arg {
            Arg::FetchSource(arg) => {
                let ret = fetch_source::run(arg).await;
                Ret::FetchSource(ret)
            },
            Arg::FetchReq(arg) => {
                let ret = fetch_req::run(arg).await;
                Ret::FetchReq(ret)
            },
            Arg::CheckProxy(arg) => {
                let ret = check_proxy::run(arg).await;
                Ret::CheckProxy(ret)
            },
            Arg::ReuseProxy(arg) => {
                let ret = reuse_proxy::run(arg).await;
                Ret::ReuseProxy(ret)
            },
        }
    }
}

mod fetch_source {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};

    pub struct Arg {
        pub client: reqwest::Client,
        pub source: String,
    }

    pub struct Ret {
        pub arg: Arg,
        pub result: Result<HashSet<String>>,
    }

    pub async fn run(arg: Arg) -> Ret {
        let response = match arg.client.get(&arg.source).send().await {
            Err(err) => return Ret {arg, result: Err(anyhow!(err))},
            Ok(response) => response,
        };
        let text = match response.text().await {
            Err(err) => return Ret {arg, result: Err(anyhow!(err))},
            Ok(text) => text,
        };
        match extract_hosts(&text) {
            Err(err) => return Ret {arg, result: Err(err)},
            Ok(hosts) => Ret {arg, result: Ok(hosts)},
        }
    }

    use std::collections::HashSet;
    use regex::Regex;
    fn extract_hosts(body: &str, ) -> Result<HashSet<String>> {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r#"(?x)
                \b(

                    (?P<ip>
                        (?: 
                            (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? ) \. 
                        ){3}

                        (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? )
                    )

                    (?:
                        : 
                    |
                        </td> \s* <td>
                    )

                    (?P<port>
                        6553[0-5] | 
                        655[0-2][0-9] | 
                        65[0-4][0-9]{2} | 
                        6[0-4][0-9]{3} | 
                        [1-5][0-9]{4} | 
                        [1-9][0-9]{1,3} 
                    )
                )\b
            "#).unwrap();
            static ref RE_HOST: Regex = Regex::new(r#"(?x)
                "host":\s*"
                    (?P<ip>
                        (?: 
                            (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? ) \. 
                        ){3}

                        (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? )
                    )
                "
            "#).unwrap();
            static ref RE_PORT: Regex = Regex::new(r#"(?x)
                "port":\s*
                (?P<port>
                    6553[0-5] | 
                    655[0-2][0-9] | 
                    65[0-4][0-9]{2} | 
                    6[0-4][0-9]{3} | 
                    [1-5][0-9]{4} | 
                    [1-9][0-9]{1,3} 
                )
            "#).unwrap();
        }
        let mut hosts: HashSet<String> = HashSet::new();
        let mut found = false;
        for caps in RE.captures_iter(&body) {
            let host = format!("{}:{}", caps.name("ip").unwrap().as_str(), caps.name("port").unwrap().as_str());
            hosts.insert(host);
            found = true;
        }
        if !found {
            for line in body.lines() {
                trace!("line: {}", line);
                if let Some(cap_host) = RE_HOST.captures(&line) {
                    trace!("cap_host: {:?}", cap_host);
                    if let Some(cap_port) = RE_PORT.captures(&line) {
                        let host = format!("{}:{}", cap_host.name("ip").unwrap().as_str(), cap_port.name("port").unwrap().as_str());
                        hosts.insert(host);
                    }
                }
            }
        }
        Ok(hosts)
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
        pub latency: Option<u128>,
    }

    pub type Ret = Arg;

    use std::time::Duration;
    pub async fn run(arg: Arg) -> Ret {
        tokio::time::delay_for(Duration::from_millis(PROXY_REST_DURATION.load(Ordering::Relaxed) as u64)).await;
        arg
    }
}

mod fetch_req {
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
        pub result: std::result::Result<RetOk, reqwest::Error>,
        pub opt: Opt,
    }

    pub struct RetOk {
        pub url: reqwest::Url,
        pub status: http::StatusCode,
        pub text: String,
        pub latency: u128,
    }

    pub struct Opt {
        pub req: Req,
        pub delivery_tag_req: LongLongUInt,
        pub url: String,
        pub success_count: usize,
    }

    pub async fn run(arg: Arg) -> Ret {
        let start = std::time::Instant::now();
        let Arg {  client, opt } = arg;
        match client.request(opt.req.method.clone(), opt.req.url.clone()).send().await {
            Err(err) => Ret { 
                result: Err(err),
                opt,
            },
            Ok(response) => {
                let url = response.url().clone();
                let status = response.status();
                match response.text().await {
                    Err(err) => Ret { 
                        result: Err(err),
                        opt,
                    },
                    Ok(text) => Ret {
                        result: Ok(RetOk{
                            url,
                            status,
                            text,
                            latency: std::time::Instant::now().duration_since(start).as_millis(),
                        }),
                        opt,
                    }
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

