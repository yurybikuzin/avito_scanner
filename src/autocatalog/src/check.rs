

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path};

use tokio::fs;
use regex::Regex;

pub type Item = String;
pub struct Arg<'a> {
    pub item: Item,
    pub out_dir: &'a Path,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ret {
    pub item_to_fetch: Option<Item>,
}

pub async fn run<'a>(arg: Arg<'a>) -> Result<Ret> {
    let file_path = super::file_spec::get(arg.out_dir, &arg.item);
    match fs::metadata(&file_path).await {
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::NotFound => {
                    lazy_static! {
                        static ref RE: Regex = Regex::new(r"/\d+$").unwrap();
                    }
                    let s = arg.item;
                    let s = RE.replace(&s, "").to_string();
                    Ok(Ret{item_to_fetch: Some(s)})
                },
                _ => Err(Error::new(err).context(format!("{:?}", &file_path))),
            }
        },
        Ok(_) => {
            Ok(Ret{item_to_fetch: None})
        },
    }
}

