
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use lapin::{ Channel };
use std::time::Duration;
use futures::{StreamExt};
use rmq::{get_conn, get_queue, Pool, basic_consume, basic_publish, basic_ack};
use serde::Serialize;
use super::*;

// ============================================================================
// ============================================================================

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "cmd";
        let consumer_tag = "cmd_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => trace!("{} listen returned", consumer_tag),
            Err(e) => error!("{} listen had an error: {}", consumer_tag, e),
        };
    }
}

#[derive(Serialize)]
struct CmdError<'a> {
    cmd: &'a str,
    msg: String,
}

pub async fn publish_load_list(channel: &Channel) -> Result<()> {
    if 
        STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_NONE ||
        STATE_PROXIES_TO_USE.load(Ordering::Relaxed) == STATE_PROXIES_TO_USE_NONE ||
        false
    {
        let queue_name = "cmd";
        let _queue = get_queue(channel, queue_name).await?;
        basic_publish(channel, queue_name, "load_list").await?;
    } else {
        trace!("cmd load_list ignored due to STATE_PROXIES_TO_CHECK/USE is not NONE");
    }
    Ok(())
}

use std::sync::atomic::{Ordering, AtomicU8};
pub static IS_LOADED_LIST: AtomicU8 = AtomicU8::new(0);//secs
async fn listen<S: AsRef<str>, S2: AsRef<str>>(pool: Pool, consumer_tag: S, queue_name: S2) -> Result<()> {
    let conn = get_conn(pool).await.map_err(|e| {
        eprintln!("could not get rmq conn: {}", e);
        e
    })?;
    let channel = conn.create_channel().await?;
    let _queue = get_queue(&channel, queue_name.as_ref()).await?;
    let mut consumer = basic_consume(&channel, queue_name.as_ref(), consumer_tag.as_ref()).await?;

    {
        if IS_LOADED_LIST.load(Ordering::Relaxed) == 0 {
            let queue_name = "proxies_to_check";
            let queue = get_queue(&channel, queue_name).await?;
            if queue.message_count() == 0 {
                let queue_name = "proxies_to_use";
                let queue = get_queue(&channel, queue_name).await?;
                if queue.message_count() < PROXIES_TO_USE_MIN_COUNT.load(Ordering::Relaxed) {
                    load_list(&channel).await?;
                    trace!("cmd load_list is sent");
                } else {
                    trace!("cmd load_list is not sent due to queue {} is full", queue_name);
                    STATE_PROXIES_TO_USE.store(STATE_PROXIES_TO_USE_FILLED, Ordering::Relaxed);
                }
            } else {
                STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_FILLED, Ordering::Relaxed);
                trace!("cmd load_list is not sent due to queue {} is not empty", queue_name);
            }
            IS_LOADED_LIST.store(1, Ordering::Relaxed);
        }
    }
    println!("{} connected, waiting for messages", consumer_tag.as_ref());
    while let Some(delivery) = consumer.next().await {
        trace!("delivery: {:?}", delivery);
        if let Ok((channel, delivery)) = delivery {

            let cmd = std::str::from_utf8(&delivery.data).unwrap();
            trace!("got {}", cmd);
            match cmd {
                "load_list" => {
                    if STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_NONE {
                        trace!("load_list");
                        STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_STARTED, Ordering::Relaxed);
                        let ret = load_list(&channel).await;
                        if STATE_PROXIES_TO_CHECK.load(Ordering::Relaxed) == STATE_PROXIES_TO_CHECK_STARTED {
                            STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_NONE, Ordering::Relaxed);
                        }
                        if let Err(err) = ret {
                            bail!(err);
                        }

                    } else {
                        trace!("cmd list ignored due to STATE_PROXIES_TO_CHECK/USE is not NONE");
                    }
                },
                _ => {
                    let queue_name = "cmd_error";
                    let _queue = get_queue(&channel, queue_name).await?;
                    let msg = format!("available commands: 'fetch_proxies'");
                    warn!("unknown command '{}', {}", cmd, msg);
                    let cmd_error = CmdError { cmd, msg };
                    let payload = serde_json::to_string_pretty(&cmd_error)?;
                    basic_publish(&channel, queue_name, payload).await?;
                },
            }
            basic_ack(&channel, delivery.delivery_tag).await?;
        }
    }
    Ok(())
}

async fn load_list(channel: &Channel) -> Result<()> {
    let mut hosts = HashSet::new();
    let sources: Vec<&str> = vec![
        "https://raw.githubusercontent.com/fate0/proxylist/master/proxy.list", // 394 => 179
        "https://raw.githubusercontent.com/a2u/free-proxy-list/master/free-proxy-list.txt", // 4 => 1
        "http://rootjazz.com/proxies/proxies.txt", // 925 => 88
        "https://socks-proxy.net/", // 300 => 3
        "http://online-proxy.ru/", // 1769 => 19,
    ];
    for source in sources {
        hosts = match fetch_url(source, hosts.clone()).await {
            Ok(hosts) => {
                trace!("fetched from {:?}", source);
                hosts
            },
            Err(err) => {
                warn!("source{:?}: {}", source, err);
                hosts
            },
        };
    }

    let sources: Vec<&str> = vec![
        // "data/list.txt",
    ];
    for source in sources {
        let file_path = Path::new(source);
        hosts = match from_file(file_path, hosts.clone()).await {
            Ok(hosts) => {
                trace!("fetched from {:?}", source);
                hosts
            },
            Err(err) => {
                warn!("source{:?}: {}", source, err);
                hosts
            },
        };
    }

    trace!("hosts.len: {}", hosts.len());
    let queue_name = "proxies_to_check";
    let queue = get_queue(&channel, queue_name).await.map_err(|err| anyhow!(err))?;
    trace!("got queue: {:?}", queue);
    for host in hosts {
        let url_proxy = format!("http://{}", host);
        if let Ok(_url_proxy) = reqwest::Proxy::all(&url_proxy) {
            basic_publish(&channel, queue_name, host).await?;
            STATE_PROXIES_TO_CHECK.store(STATE_PROXIES_TO_CHECK_FILLED, Ordering::Relaxed);
        }
    }
    Ok(())
}

use tokio::fs;
use tokio::prelude::*;
use std::path::Path;
async fn from_file(file_path: &Path, hosts: HashSet<String>) -> Result<HashSet<String>> {
    let mut file = fs::File::open(file_path).await?;
    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;
    let body = std::str::from_utf8(&contents).unwrap();
    extract_hosts(body, hosts)
}

use regex::Regex;
use lazy_static::lazy_static;
use std::collections::HashSet;
async fn fetch_url(url: &str, hosts: HashSet<String>) -> Result<HashSet<String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(PROXY_TIMEOUT.load(Ordering::Relaxed) as u64))
        .build()?
    ;
    let body = client.get(url).send()
        .await?
        .text()
        .await?
    ;
    extract_hosts(&body, hosts)
}

fn extract_hosts(body: &str, mut hosts: HashSet<String>) -> Result<HashSet<String>> {
    lazy_static! {
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
    let mut found = false;
    for caps in RE.captures_iter(&body) {
        let host = format!("{}:{}", caps.name("ip").unwrap().as_str(), caps.name("port").unwrap().as_str());
        info!("host: {}", host);
        hosts.insert(host);
        found = true;
    }
    trace!("found: {}", found);
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


