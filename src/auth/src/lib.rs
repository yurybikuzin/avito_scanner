
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::time::Instant;
use http::StatusCode;

// ============================================================================
// ============================================================================

const AUTH: &str = "AVITO_AUTH"; 
const SERVICE_URL: &str = "http://auth:3000";

pub type StringProvider = fn() -> Result<String>;

pub struct Arg {
    pub init: Option<StringProvider>,
    pub fini: Option<StringProvider>,
}

impl Arg {
    pub fn new() -> Self {
        Self { init: None, fini: None }
    }
    pub fn init(mut self, sp: StringProvider) -> Self {
        self.init = Some(sp);
        self
    }
    pub fn fini(mut self, sp: StringProvider) -> Self {
        self.fini = Some(sp);
        self
    }
}

pub async fn get(arg: Option<Arg>) -> Result<String> {
    match std::env::var(AUTH) {
        Ok(auth) => {             
            info!("from {}, auth::get: {}", AUTH, auth);
            Ok(auth)
        },
        Err(_) => {
            if let Some(arg) = &arg {
                match arg.init {
                    None => println!("Получение токена авторизации . . ."),
                    Some(cb) => {
                        let s = cb()?;
                        println!("{}", s);
                    },
                }
            }
            let start = Instant::now();
            let response = reqwest::get(SERVICE_URL).await?;
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
            if let Some(arg) = arg {
                match arg.init {
                    None => println!("{}, Токен авторизации получен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), auth),
                    Some(cb) => {
                        let s = cb()?;
                        println!("{}", s);
                    },
                }
            } else {
                info!("{}, auth::get: {}", 
                    arrange_millis::get(Instant::now().duration_since(start).as_millis()), 
                    auth,
                );
            }
            Ok(auth) // af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir
        },
    }
}

pub struct Lazy {
    arg: Option<Arg>,
    key: Option<String>,
}

impl Lazy {
    pub fn new(arg: Option<Arg>) -> Self { // arg=Some(key::Arg::new())
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
                let mut arg: Option<Arg> = None;
                std::mem::swap(&mut arg, &mut self.arg);
                let key = get(arg).await?;
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
    async fn test_get() -> Result<()> {
        test_helper::init();

        let _ = get(Some(Arg::new())).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_lazy() -> Result<()> {
        test_helper::init();

        let mut auth = Lazy::new(Some(Arg::new()));
        assert_eq!(auth.key, None);

        let key = auth.key().await?;
        info!("key: {}", key);

        let key2 = auth.key().await?;
        assert_eq!(key2, key);

        Ok(())
    }
}

