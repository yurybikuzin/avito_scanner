
macro_rules! let_settings {
    ($settings: ident) => {
        let $settings = settings::SINGLETON.read().unwrap();
        let $settings = $settings.as_ref().unwrap();
    };
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

macro_rules! fut_fetch {
    ($fut_queue: expr, $fetch_req_count: ident, $req: expr, $url: expr, $proxy: expr, $success_count: expr, $delivery_tag_req: expr) => {
        let response_timeout_secs = settings::SINGLETON.read().unwrap().as_ref().unwrap().response_timeout_secs;
        let client = reqwest::Client::builder()
            .proxy($proxy)
            .timeout(
                $req.timeout.unwrap_or(
                    Duration::from_secs(response_timeout_secs)
                )
            )
            .build()?
        ;
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
    ($fut_queue: expr, $source_fetch_count: ident, $proxy_url_queue_to_check: expr, $proxy_url_check_count: expr, $source_queue_to_fetch: expr) => {
        if $source_queue_to_fetch.len() == 0 && $source_fetch_count == 0 && $proxy_url_queue_to_check.len() == 0 && $proxy_url_check_count == 0 {
            // начинаем со чтения списка url'ов источников
            let_settings!(settings);
            for source in settings.sources.iter() {
                $source_queue_to_fetch.push_back(source.to_owned());
            }
            info!("Заполнили очередь источников прокси: {}", $source_queue_to_fetch.len());
            // пока есть вакантные места на fetch источников, заполняем их
            while $source_fetch_count < settings.same_time_request_max_count { 
                if let Some(source) = $source_queue_to_fetch.pop_front() {
                    let client = reqwest::Client::builder()
                        .timeout(Duration::from_secs(settings.proxy_timeout_secs))
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
            start_proxy_check!($fut_queue, $source_fetch_count, $proxy_url_queue_to_check, $proxy_url_check_count, $source_queue_to_fetch);
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
    ($fut_queue: expr, $url: expr, $own_ip: ident) => {
        let proxy = match reqwest::Proxy::all(&$url) {
            Ok(proxy) => proxy,
            Err(err) => {
                bail!("malformed url {}: {}", $url, err);
            },
        };
        let_settings!(settings);
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(settings.proxy_timeout_secs))
            .build()?
        ;
        let opt = check_proxy::Opt { url: $url };
        $fut_queue.push(op::run(op::Arg::CheckProxy(check_proxy::Arg { 
            client, 
            own_ip: $own_ip.to_owned(), 
            opt,
            echo_service_url: settings.echo_service_url.to_owned(),
        })));
    };
}

macro_rules! fut_reuse_proxy {
    ($fut_queue: expr, $reuse_proxy_count: ident, $url: expr, $latency: expr, $success_count: expr) => {
        let_settings!(settings);
        $fut_queue.push(op::run(op::Arg::ReuseProxy(reuse_proxy::Arg {
            url: $url,
            success_count: $success_count,
            latency: $latency,
            delay_millis: settings.proxy_rest_duration_millis,
        })));
        $reuse_proxy_count += 1;
    };
}

macro_rules! fut_update_own_ip {
    ($fut_queue: expr, $own_ip: expr, $own_ip_client: expr) => {
        let_settings!(settings);
        $fut_queue.push(op::run(op::Arg::UpdateOwnIp(update_own_ip::Arg{
            own_ip: $own_ip.clone(), 
            client: $own_ip_client.clone(),
            delay_secs: settings.own_ip_fresh_duration_secs,
            echo_service_url: settings.echo_service_url.to_owned(),
        })));
    };
}
