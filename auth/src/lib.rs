
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::time::Instant;
use http::StatusCode;

// ============================================================================
// ============================================================================

const AUTH: &str = "AVITO_AUTH"; 

pub async fn get() -> Result<String> {
    match std::env::var(AUTH) {
        Ok(auth) => {             info!("from {}, auth::get: {}", 
                AUTH, 
                auth,
            );
            Ok(auth)
        },
        Err(_) => {
            let now = Instant::now();
            let response = reqwest::get("http://auth:3000").await?;
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
            info!("{} ms, auth::get: {}", 
                Instant::now().duration_since(now).as_millis(), 
                auth,
            );
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
    async fn to_file() -> Result<()> {
        init();

        let _ = get().await?;

        Ok(())
    }
}

