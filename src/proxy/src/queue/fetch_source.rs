
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

pub struct Arg {
    pub client: reqwest::Client,
    pub source: String,
}

pub struct Ret {
    pub arg: Arg,
    pub result: Result<HashSet<String>>,
}

pub async fn run(arg: Arg) -> Ret {
    let response = match arg.client.get(&arg.source).send().await {
        Err(err) => return Ret {arg, result: Err(anyhow!(err))},
        Ok(response) => response,
    };
    let text = match response.text().await {
        Err(err) => return Ret {arg, result: Err(anyhow!(err))},
        Ok(text) => text,
    };
    match extract_hosts(&text) {
        Err(err) => return Ret {arg, result: Err(err)},
        Ok(hosts) => Ret {arg, result: Ok(hosts)},
    }
}

use std::collections::HashSet;
use regex::Regex;
fn extract_hosts(body: &str, ) -> Result<HashSet<String>> {
    lazy_static::lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
            \b(

                (?P<ip>
                    (?: 
                        (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? ) \. 
                    ){3}

                    (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? )
                )

                (?:
                    : 
                |
                    </td> \s* <td>
                )

                (?P<port>
                    6553[0-5] | 
                    655[0-2][0-9] | 
                    65[0-4][0-9]{2} | 
                    6[0-4][0-9]{3} | 
                    [1-5][0-9]{4} | 
                    [1-9][0-9]{1,3} 
                )
            )\b
        "#).unwrap();
        static ref RE_HOST: Regex = Regex::new(r#"(?x)
            "host":\s*"
                (?P<ip>
                    (?: 
                        (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? ) \. 
                    ){3}

                    (?: 25[0-5] | 2[0-4][0-9] | [01]?[0-9][0-9]? )
                )
            "
        "#).unwrap();
        static ref RE_PORT: Regex = Regex::new(r#"(?x)
            "port":\s*
            (?P<port>
                6553[0-5] | 
                655[0-2][0-9] | 
                65[0-4][0-9]{2} | 
                6[0-4][0-9]{3} | 
                [1-5][0-9]{4} | 
                [1-9][0-9]{1,3} 
            )
        "#).unwrap();
    }
    let mut hosts: HashSet<String> = HashSet::new();
    let mut found = false;
    for caps in RE.captures_iter(&body) {
        let host = format!("{}:{}", caps.name("ip").unwrap().as_str(), caps.name("port").unwrap().as_str());
        hosts.insert(host);
        found = true;
    }
    if !found {
        for line in body.lines() {
            trace!("line: {}", line);
            if let Some(cap_host) = RE_HOST.captures(&line) {
                trace!("cap_host: {:?}", cap_host);
                if let Some(cap_port) = RE_PORT.captures(&line) {
                    let host = format!("{}:{}", cap_host.name("ip").unwrap().as_str(), cap_port.name("port").unwrap().as_str());
                    hosts.insert(host);
                }
            }
        }
    }
    Ok(hosts)
}
