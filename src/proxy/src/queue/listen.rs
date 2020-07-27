
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::collections::VecDeque;
use std::time::Duration;
use rmq::{
    get_conn, 
    get_queue, 
    Pool, 
    basic_consume, 
    basic_publish, 
    basic_ack,
};

use super::*;

use req::Req;
use res::Res;

use futures::{
    StreamExt,
    channel::mpsc::{self, Receiver, Sender},
    select,
    pin_mut,
    stream::{
        FuturesUnordered,
    },
};

use std::collections::{HashSet, HashMap};
use super::settings;

struct ReqQueueItem {
    req: Req,
    delivery_tag: amq_protocol_types::LongLongUInt,
}

struct ProxyUrlQueueItem {
    url: String,
    success_count: usize,
    latency: Option<u128>,
}

pub async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
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

    let protos = vec!["socks5", "http", "https"];
    let mut proxy_url_queue_to_check = VecDeque::<String>::new();
    let mut proxy_url_check_count = 0usize;
    let mut source_queue_to_fetch = VecDeque::<String>::new();
    let mut source_fetch_count = 0usize;
    let own_ip_client = {
        let_settings!(settings);
        reqwest::Client::builder()
            .timeout(Duration::from_secs(settings.proxy_timeout_secs))
            .build()?
    };
    let mut own_ip: String = {
        let_settings!(settings);
        get_ip( own_ip_client.clone(), settings.echo_service_url.as_ref()).await?
    };
    info!("own_ip: {}", own_ip);
    {
        fut_update_own_ip!(fut_queue, own_ip, own_ip_client);
    }
    pub struct Checked {
        pub status: check_proxy::Status,
        pub url: String,
    }
    let mut checking = HashMap::<String, Vec<Checked>>::new();
    let mut fetch_req_count = 0usize;
    let mut reuse_proxy_count = 0usize;

    start_proxy_check!(fut_queue, source_fetch_count, proxy_url_queue_to_check, proxy_url_check_count, source_queue_to_fetch);
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
                    op::Ret::UpdateOwnIp(own_ip_updated) => {
                        own_ip = own_ip_updated;
                        fut_update_own_ip!(fut_queue, own_ip, own_ip_client);
                    },
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
                                let_settings!(settings);
                                // let settings = settings::SINGLETON.read().unwrap().as_ref().unwrap();
                                while proxy_url_check_count < settings.same_time_proxy_check_max_count {
                                    match proxy_url_queue_to_check.pop_front() {
                                        None => break,
                                        Some(url) => {
                                            fut_push_check_proxy!(fut_queue, url, own_ip);
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
                                    let_settings!(settings);
                                    // let settings = settings::SINGLETON.read().unwrap().as_ref().unwrap();
                                    let success_count = settings.success_start_count;
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
                            fut_push_check_proxy!(fut_queue, url, own_ip);
                        } else {
                            proxy_url_check_count -= 1;
                        }
                        trace!("Сейчас одноврменно проверяется прокси: {}", proxy_url_check_count);
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

                                        info!("формируем отложенную просьбу на возврат прокси {} в очередь прокси с latency {} и увеличением кредита доверия до {}", url, latency, success_count);
                                        let_settings!(settings);
                                        let success_count = if success_count >= settings.success_max_count { 
                                                success_count 
                                            } else { 
                                                success_count + 1 
                                        };
                                        fut_reuse_proxy!(fut_queue, reuse_proxy_count, url, Some(latency), success_count);
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

