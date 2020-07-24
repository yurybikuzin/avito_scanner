
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use rmq::Pool;
use req::Req;
use super::settings;
use serde::Serialize;

#[derive(Serialize)]
struct ProxyError {
    url: String,
    msg: String,
}

mod op;
mod get_ip;
mod get_host_of_url;
mod check_proxy;
mod update_own_ip; 
mod reuse_proxy;
mod fetch_req;
mod fetch_source; 
#[macro_use]
mod macros;
mod listen;

use get_ip::get_ip;
use get_host_of_url::get_host_of_url;

pub async fn process(pool: Pool) -> Result<()> {
    let mut retry_interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        retry_interval.tick().await;
        let queue_name = "request";
        let consumer_tag = "request_consumer";
        println!("connecting {} ...", consumer_tag);
        match listen::listen(pool.clone(), consumer_tag, queue_name).await {
            Ok(_) => println!("{} listen returned", consumer_tag),
            Err(e) => eprintln!("{} listen had an error: {}", consumer_tag, e),
        };
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

    // docker exec -it -e RUST_LOG=diaps=trace -e AVITO_AUTH=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir avito-proj cargo test -p diaps -- --nocapture
    #[tokio::test]
    async fn test_get_host_of_proxy_url() -> Result<()> {
        test_helper::init();

        let url = "http://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        let url = "socks5://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        let url = "https://91.121.175.66:35391";
        assert_eq!(get_host_of_url(url).unwrap(), "91.121.175.66:35391");

        Ok(())
    }

}

