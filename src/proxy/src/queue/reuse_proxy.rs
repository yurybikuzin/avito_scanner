
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

pub struct Arg {
    pub url: String,
    pub success_count: usize,
    pub latency: Option<u128>,
    pub delay_millis: u64,
}

pub struct Ret {
    pub url: String,
    pub success_count: usize,
    pub latency: Option<u128>,
}

use std::time::Duration;
pub async fn run(arg: Arg) -> Ret {
    tokio::time::delay_for(Duration::from_millis(arg.delay_millis)).await;
    Ret {
        url: arg.url,
        success_count: arg.success_count,
        latency: arg.latency,
    }
}
