#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use super::*;

pub struct Arg {
    pub own_ip: String,
    pub client: reqwest::Client,
    pub delay_secs: u64,
    pub echo_service_url: String,
}

pub type Ret = String;

use std::time::Duration;
pub async fn run(arg: Arg) -> Ret {
    tokio::time::delay_for(Duration::from_secs(arg.delay_secs)).await;
    match get_ip( arg.client, arg.echo_service_url.as_ref()).await {
        Ok(own_ip) => {
            if own_ip != arg.own_ip {
                info!("own_ip changed: {} => {}", arg.own_ip, own_ip);
            }
            own_ip
        },
        Err(err) => {
            warn!("old own_ip {} will be used due to {}", arg.own_ip, err);
            arg.own_ip
        },
    }
}
