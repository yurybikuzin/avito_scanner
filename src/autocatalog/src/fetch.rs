
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;
// use http::StatusCode;
// use serde_json::{Value, Number};

// use regex::Regex;

type Item = str;

pub struct Arg<I: AsRef<Item>> {
    // pub client: reqwest::Client,
    pub client: client::Client,
    pub item: I,
    // pub retry_count: usize,
}

impl<I: AsRef<Item>> Arg<I> {
    pub async fn url(&self) -> Result<Url> {
        let url = &format!("https://avito.ru{}", 
            self.item.as_ref(),
        );
        let url = Url::parse(&url)?;
        Ok(url)
    }
}

use super::fetched::{Fetched};
// use super::card::{Card, Record};

pub struct Ret<I: AsRef<Item>> {
    // pub client: reqwest::Client,
    pub client: client::Client,
    pub item: I,
    pub fetched: Fetched,
}


// const SLEEP_TIMEOUT: u64 = 500;
//
// use std::{thread, time};

pub async fn run<I: AsRef<Item>>(arg: Arg<I>) -> Result<Ret<I>> {
    let url = arg.url().await?;
    info!("url: {:?}", url);

    let (text, status) = arg.client.get_text_status(url.clone()).await.context("cards::fetch")?;
    match status {
        code @ http::StatusCode::NOT_FOUND => {
            warn!("{} :: {}: {}", url, code, text);
            return Ok(Ret {
                client: arg.client,
                item: arg.item, 
                fetched: Fetched::NotFound,
            })
        },
        http::StatusCode::OK => {
        },
        code @ _ => {
            return Err(anyhow!("{} :: {}: {}", url, code, text));
        },
    }

    // let text = {
    //     let text: Result<String>;
    //     let mut remained = arg.retry_count;
    //     loop {
    //         let response = arg.client.get(url.clone()).send().await;
    //         match response {
    //             Err(err) => {
    //                 if remained > 0 {
    //                     remained -= 1;
    //                     let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
    //                     thread::sleep(duration);
    //                     continue;
    //                 } else {
    //                     error!("{}: {:?}", url, err);
    //                     text = Err(Error::new(err));
    //                     break;
    //                 }
    //             },
    //             Ok(response) => {
    //                 match response.status() {
    //                     StatusCode::OK => {
    //                         match response.text().await {
    //                             Ok(t) => {
    //                                 text = Ok(t);
    //                                 break;
    //                             },
    //                             Err(err) => {
    //                                 if remained > 0 {
    //                                     remained -= 1;
    //                                     let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
    //                                     thread::sleep(duration);
    //                                     continue;
    //                                 } else {
    //                                     error!("{}: {:?}", url, err);
    //                                     text = Err(Error::new(err));
    //                                     break;
    //                                 }
    //                             },
    //                         }
    //                     },
    //                     code @ StatusCode::NOT_FOUND => {
    //                         let msg = response.text().await?;
    //                         warn!("{} :: {}: {}", url, code, msg);
    //                         return Ok(Ret {
    //                             client: arg.client,
    //                             item: arg.item, 
    //                             fetched: Fetched::NotFound,
    //                         })
    //                     },
    //                     code @ _ => {
    //                         if remained > 0 {
    //                             remained -= 1;
    //                             let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
    //                             thread::sleep(duration);
    //                             continue;
    //                         } else {
    //                             let msg = response.text().await?;
    //                             error!("{} :: {}: {}", url, code, msg);
    //                             text = Err(anyhow!("{} :: {}: {}", url, code, msg));
    //                             break;
    //                         }
    //                     },
    //                 }
    //             }
    //         }
    //     }
    //     text
    // }.context("cards::fetch")?;

    let fetched = Fetched::parse(text, url)?;

    Ok(Ret {
        client: arg.client,
        item: arg.item, 
        fetched,
    })
}

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

    #[tokio::test]
    async fn test_fetch_only() -> Result<()> {
        init();

        let pool = rmq::get_pool();
        let client_provider = client::Provider::new(client::Kind::ViaProxy(pool));
        let client = client_provider.build().await?;
        let arg = Arg {
            client: client,
            item: "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan",
        };

        let ret = run(arg).await?;
        match ret.fetched {
            Fetched::Records(vec_record) => {
                assert_eq!(vec_record.len(), 31);
            },
            _ => unreachable!(),
        }
        Ok(())
    }
}
