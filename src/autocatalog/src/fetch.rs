
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
    pub client: client::Client,
    pub item: I,
}

impl<I: AsRef<Item>> Arg<I> {
    pub async fn url(&self) -> Result<Url> {
        let url = &format!("https://www.avito.ru{}", 
            self.item.as_ref(),
        );
        let url = Url::parse(&url)?;
        Ok(url)
    }
}

use super::fetched::{Fetched};

pub struct Ret<I: AsRef<Item>> {
    pub client: client::Client,
    pub item: I,
    pub fetched: Fetched,
}


pub async fn run<I: AsRef<Item>>(arg: Arg<I>) -> Result<Ret<I>> {
    let url = arg.url().await?;

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

    let fetched = Fetched::parse(text, url).await?;

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

        let settings = rmq::Settings::new(std::path::Path::new("../../cnf/rmq/bikuzin18.toml"))?;
        let pool = rmq::get_pool(settings)?;
        let client_provider = client::Provider::new(client::Kind::ViaProxy(pool, "autocatalog".to_owned()));
        let client = client_provider.build().await?;
        let arg = Arg {
            client: client,
            item: "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan",
        };

        let ret = run(arg).await?;
        match ret.fetched {
            Fetched::Records(vec_record) => {
                assert_eq!(vec_record.len(), 31);
                info!("vec_record: {}", serde_json::to_string_pretty(&vec_record)?);
            },
            _ => unreachable!(),
        }
        Ok(())
    }
}
