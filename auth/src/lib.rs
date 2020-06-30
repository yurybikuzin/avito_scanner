
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

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    #[tokio::test]
    async fn it_works() -> Result<()> {
        init();

        let _ = get(Some(&Arg::new())).await?;

        Ok(())
    }
}

