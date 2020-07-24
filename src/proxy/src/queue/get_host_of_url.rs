
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use lazy_static::lazy_static;
use regex::Regex;
pub fn get_host_of_url(url: &str) -> Result<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
            ^
            (?P<proto>.*?)
            ://
            (?P<host>.*)
            $
        "#).unwrap();
    }
    match RE.captures(url) {
        Some(caps) => Ok(caps.name("host").unwrap().as_str().to_owned()),
        None => Err(anyhow!("Failed to extract host from url: {}", url)),
    }
}
