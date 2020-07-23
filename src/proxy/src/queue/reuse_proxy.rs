
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use super::super::*;

pub struct Arg {
    pub url: String,
    pub success_count: usize,
    pub latency: Option<u128>,
}

pub type Ret = Arg;

use std::time::Duration;
pub async fn run(arg: Arg) -> Ret {
    tokio::time::delay_for(Duration::from_millis(PROXY_REST_DURATION.load(Ordering::Relaxed) as u64)).await;
    arg
}
