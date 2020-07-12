
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;

pub struct Arg {
    pub client: client::Client,
    pub auth: String,
    pub id: u64,
}

use super::fetched::{Fetched};

pub struct Ret {
    pub client: client::Client,
    pub id: u64,
    pub fetched: Fetched,
}

pub async fn run(arg: Arg) -> Result<Ret> {
    let url = &format!("https://avito.ru/api/14/items/{}?key={}", 
        arg.id,
        arg.auth, 
    );
    let url = Url::parse(&url)?;
    let (text, status) = arg.client.get_text_status(url.clone()).await.context("cards::fetch")?;
    match status {
        code @ http::StatusCode::NOT_FOUND => {
            warn!("{} :: {}: {}", url, code, text);
            return Ok(Ret {
                client: arg.client,
                id: arg.id, 
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
        id: arg.id, 
        fetched,
    })
}

