
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};
use http::StatusCode;

pub enum Client {
    Reqwest(Wrapper),
    ViaProxy(via_proxy::Client),
}

#[derive(Clone)]
pub enum Kind {
    Reqwest(usize),
    ViaProxy(rmq::Pool, String),
}

#[derive(Clone)]
pub struct Provider {
    kind: Kind
}

impl Provider {
    pub fn new(kind: Kind) -> Self {
        Self { kind }
    }
    pub async fn build(&self) -> Result<Client> {
        match &self.kind {
            Kind::Reqwest(retry_count) => {
                Ok( Client::Reqwest(Wrapper {
                    retry_count: *retry_count,
                    client: reqwest::Client::new(),
                }) )
            },
            Kind::ViaProxy(poll, queue_name) => {
                Ok( Client::ViaProxy( via_proxy::Client::new(poll.clone(), queue_name.to_owned()).await? ) )
            },
        }
    }
}

impl Client {
    pub async fn get_text_status(&self, url: url::Url) -> Result<(String, http::StatusCode)> {
        Ok(match self {
            Client::Reqwest(client) => client.get_text_status(url).await?,
            Client::ViaProxy(client) => client.get_text_status(url).await?,
        })
    }
}

pub struct Wrapper {
    retry_count: usize,
    client: reqwest::Client,
}

impl Wrapper {
    async fn get_text_status(&self, url: url::Url) -> Result<(String, http::StatusCode)> {
        let (text, status) = {
            // let text: Result<String>;
            let text: String;
            let status: http::StatusCode;
            let mut remained = self.retry_count;
            loop {
                let response = self.client.get(url.clone()).send().await;
                match response {
                    Err(err) => {
                        if remained > 0 {
                            remained -= 1;
                            continue;
                        } else {
                            return Err(Error::new(err))
                        }
                    },
                    Ok(response) => {
                        match response.status() {
                            code @ StatusCode::OK => {
                                match response.text().await {
                                    Ok(t) => {
                                        // text = Ok(t);
                                        text = t;
                                        status = code;
                                        break;
                                    },
                                    Err(err) => {
                                        if remained > 0 {
                                            remained -= 1;
                                            continue;
                                        } else {
                                            return Err(Error::new(err))
                                        }
                                    },
                                }
                            },
                            code @ _ => {
                                if remained > 0 {
                                    remained -= 1;
                                    continue;
                                } else {
                                    // text = Ok(response.text().await?);
                                    text = response.text().await?;
                                    status = code;
                                    // (text, status)
                                    // text = Err(anyhow!("{}: {}", code, response.text().await?));
                                    break;
                                }
                            },
                        }
                    }
                }
            }
            (text, status)
        };
        // }.context("ids::fetch")?;
        Ok((text, status))
    }
}


//
// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
