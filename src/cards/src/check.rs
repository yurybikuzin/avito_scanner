

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path};

use tokio::fs;

pub struct Arg<'a> {
    pub id: u64,
    pub out_dir: &'a Path,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ret {
    pub id: Option<u64>,
}

pub async fn run<'a>(arg: Arg<'a>) -> Result<Ret> {
    let file_path = super::file_spec::get(arg.out_dir, arg.id);
    match fs::metadata(&file_path).await {
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::NotFound => Ok(Ret{id: Some(arg.id)}),
                _ => Err(Error::new(err).context(format!("{:?}", &file_path))),
            }
        },
        Ok(_) => {
            Ok(Ret{id: None})
        },
    }
}

