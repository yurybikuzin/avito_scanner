
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path, PathBuf};

type Item = str;
pub fn get<I: AsRef<Item>>(out_dir: &Path, item: I) -> PathBuf {
    let s = format!("{}{}", out_dir.to_string_lossy(), item.as_ref());
    let path = PathBuf::from(s);
    path.with_extension("json")
}

