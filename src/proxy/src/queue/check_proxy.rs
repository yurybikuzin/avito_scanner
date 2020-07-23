
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

pub struct Arg {
    pub client: reqwest::Client,
    pub own_ip: String,
    pub opt: Opt,
}

pub struct Ret {
    pub status: Status,
    pub opt: Opt,
}

pub struct Opt {
    pub url: String,
    // pub host: String,
}

pub enum Status {
    Ok{latency: u128},
    NonAnon,
    Err(Error),
}

pub async fn run(arg: Arg) -> Ret {
    let start = std::time::Instant::now();
    let status = match super::get_ip(arg.client).await {
        Err(err) => Status::Err(err),
        Ok(ip) => if ip != arg.own_ip {
            Status::Ok{ latency: std::time::Instant::now().duration_since(start).as_millis() }
        } else {
            Status::NonAnon
        },
    };
    Ret{
        status,
        opt: arg.opt,
    }
}
