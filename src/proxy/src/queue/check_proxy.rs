#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

// use super::get_ip::get_ip;

use super::*;

pub struct Arg {
    pub client: reqwest::Client,
    pub own_ip: String,
    pub opt: Opt,
    pub echo_service_url: String,
}

pub struct Ret {
    pub status: Status,
    pub opt: Opt,
}

pub struct Opt {
    pub url: String,
}

pub enum Status {
    Ok{latency: u128},
    NonAnon,
    Err(Error),
}

pub async fn run(arg: Arg) -> Ret {
    let start = std::time::Instant::now();
    let status = match get_ip(arg.client, arg.echo_service_url.as_ref()).await {
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
