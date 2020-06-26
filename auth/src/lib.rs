use anyhow::{bail, Result};
use http::StatusCode;

pub async fn get() -> Result<String> {
    let response = reqwest::get("http://auth:3000").await?;
    let key = match response.status() {
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
    Ok(key)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

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

        let now = Instant::now();

        let key = get().await?;

        info!("({} ms) key!: {}", 
            Instant::now().duration_since(now).as_millis(), 
            key,
        );

        Ok(())
    }
}
