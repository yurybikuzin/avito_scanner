
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::time::Instant;
use http::StatusCode;

// ============================================================================
// ============================================================================

// const AUTH: &str = "AVITO_AUTH"; 
// const SERVICE_URL: &str = "http://auth:3000";

pub type StringProvider = fn() -> Result<String>;

pub enum Arg {
    Ready {
        key: String
    },
    Lazy {
        url: String,
        init: Option<StringProvider>,
        fini: Option<StringProvider>,
    }
}

// pub struct Arg {
//     pub init: Option<StringProvider>,
//     pub fini: Option<StringProvider>,
// }

impl Arg {
    pub fn new_lazy(url: String) -> Self {
        Self::Lazy { url, init: None, fini: None }
    }
    pub fn new_ready(key: String) -> Self {
        Self::Ready { key }
    }
    pub fn init_lazy(self, sp: StringProvider) -> Self {
        match self {
            Self::Ready { key: _ } => self,
            Self::Lazy { url, init: _, fini } => {
                Self::Lazy {
                    url, 
                    init: Some(sp),
                    fini,
                }
            }
        }
    }
    pub fn fini_lazy(self, sp: StringProvider) -> Self {
        match self {
            Self::Ready { key: _ } => self,
            Self::Lazy { url, init, fini: _ } => {
                Self::Lazy {
                    url, 
                    fini: Some(sp),
                    init,
                }
            }
        }
    }
}

pub async fn get(arg: &Arg) -> Result<String> {
    match arg {
        Arg::Ready { key } => return Ok(key.to_owned()),
        Arg::Lazy { url, init, fini } => {

            match init {
                None => println!("Получение токена авторизации . . ."),
                Some(cb) => {
                    let s = cb()?;
                    println!("{}", s);
                },
            }

            let start = Instant::now();
            let response = reqwest::get(url).await?;
            let auth = match response.status() {
                StatusCode::OK => {
                    response.text().await?
                },
                StatusCode::NOT_FOUND => {
                    bail!("auth: NOT_FOUND: {}", response.text().await?);
                },
                StatusCode::INTERNAL_SERVER_ERROR => {
                    bail!("auth: INTERNAL_SERVER_ERROR: {}", response.text().await?);
                },
                _ => {
                    unreachable!();
                },
            };
            match fini {
                None => println!("{}, Токен авторизации получен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), auth),
                Some(cb) => {
                    let s = cb()?;
                    println!("{}", s);
                },
            }
            Ok(auth) // af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir

        },
    }
}

pub struct Lazy {
    arg: Arg,
    key: Option<String>,
}

impl Lazy {
    pub fn new(arg: Arg) -> Self { 
        Self {
            arg,
            key: None,
        }
    }
    pub async fn key(&mut self) -> Result<String> {
        match &self.key {
            Some(key) => {
                Ok(key.to_owned())
            }
            None => {
                let key = get(&self.arg).await?;
                self.key = Some(key.to_owned());
                Ok(key)
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

    #[tokio::test]
    async fn test_lazy() -> Result<()> {
        test_helper::init();

        let mut auth = Lazy::new(Arg::new_ready("af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir".to_owned()));
        assert_eq!(auth.key, None);

        let key = auth.key().await?;
        info!("key: {}", key);

        let key2 = auth.key().await?;
        assert_eq!(key2, key);

        Ok(())
    }
}

