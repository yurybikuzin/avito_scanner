
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use super::*;

pub enum Arg {
    UpdateOwnIp(update_own_ip::Arg),
    FetchSource(fetch_source::Arg),
    CheckProxy(check_proxy::Arg),
    FetchReq(fetch_req::Arg),
    ReuseProxy(reuse_proxy::Arg),
}

pub enum Ret {
    UpdateOwnIp(update_own_ip::Ret),
    FetchSource(fetch_source::Ret),
    CheckProxy(check_proxy::Ret),
    FetchReq(fetch_req::Ret),
    ReuseProxy(reuse_proxy::Ret),
}

pub async fn run(arg: Arg) -> Ret {
    match arg {
        Arg::UpdateOwnIp(arg) => {
            let ret = update_own_ip::run(arg).await;
            Ret::UpdateOwnIp(ret)
        },
        Arg::FetchSource(arg) => {
            let ret = fetch_source::run(arg).await;
            Ret::FetchSource(ret)
        },
        Arg::FetchReq(arg) => {
            let ret = fetch_req::run(arg).await;
            Ret::FetchReq(ret)
        },
        Arg::CheckProxy(arg) => {
            let ret = check_proxy::run(arg).await;
            Ret::CheckProxy(ret)
        },
        Arg::ReuseProxy(arg) => {
            let ret = reuse_proxy::run(arg).await;
            Ret::ReuseProxy(ret)
        },
    }
}
