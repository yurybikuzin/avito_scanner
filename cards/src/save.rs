
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::Path;
use serde_json::{Value};

pub struct Arg<'a> {
    pub id: u64,
    pub json: Value,
    pub out_dir: &'a Path,
}

pub struct Ret ();

pub async fn run<'a>(arg: Arg<'a>) -> Result<Ret> {
    todo!();
}

