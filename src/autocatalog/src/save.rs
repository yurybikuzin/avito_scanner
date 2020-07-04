
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path};
// use serde_json::{Value};

use tokio::fs::{self, File};
use tokio::prelude::*;

use super::card::Card;

type Item = str;
pub struct Arg<'a, I: AsRef<Item>> {
    pub item: I,
    pub card: Card,
    pub out_dir: &'a Path,
}

pub struct Ret ();

pub async fn run<'a, I: AsRef<Item>>(arg: Arg<'a, I>) -> Result<Ret> {
    let file_path = super::file_spec::get(arg.out_dir, arg.item);
    if let Some(dir_path) = file_path.parent() {
        fs::create_dir_all(dir_path).await?;
    }

    let mut file = File::create(file_path).await?;
    let json = serde_json::to_string_pretty(&arg.card)?;
    file.write_all(json.as_bytes()).await?;

    Ok(Ret())
}

