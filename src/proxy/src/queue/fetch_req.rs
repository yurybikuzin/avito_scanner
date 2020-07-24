#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};
use amq_protocol_types::LongLongUInt;

use super::{Req};

pub struct Arg {
    pub client: reqwest::Client,
    pub opt: Opt,
}

pub struct Ret {
    pub result: std::result::Result<RetOk, reqwest::Error>,
    pub opt: Opt,
}

pub struct RetOk {
    pub url: reqwest::Url,
    pub status: http::StatusCode,
    pub text: String,
    pub latency: u128,
}

pub struct Opt {
    pub req: Req,
    pub delivery_tag_req: LongLongUInt,
    pub url: String,
    pub success_count: usize,
}

pub async fn run(arg: Arg) -> Ret {
    let start = std::time::Instant::now();
    let Arg {  client, opt } = arg;
    match client.request(opt.req.method.clone(), opt.req.url.clone()).send().await {
        Err(err) => Ret { 
            result: Err(err),
            opt,
        },
        Ok(response) => {
            let url = response.url().clone();
            let status = response.status();
            match response.text().await {
                Err(err) => Ret { 
                    result: Err(err),
                    opt,
                },
                Ok(text) => Ret {
                    result: Ok(RetOk{
                        url,
                        status,
                        text,
                        latency: std::time::Instant::now().duration_since(start).as_millis(),
                    }),
                    opt,
                }
            }
        }
    }
}
