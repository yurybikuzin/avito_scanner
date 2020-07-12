
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{PathBuf};

use tokio::{
    fs::{File},
};
use tokio::prelude::*; // for read_to_end()

pub struct Arg {
    pub file_path: PathBuf,
}

use std::str;

pub type Ret = cards::Fetched;
pub async fn run(arg: Arg) -> Result<Ret> {
    let mut file = File::open(&arg.file_path).await?;
    let mut contents = vec![];
    file.read_to_end(&mut contents).await?;
    let s = str::from_utf8(&contents)?;
    let ret: Ret = serde_json::from_str(s)?;

    Ok(ret)
}

