
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{
    // Path, 
    PathBuf};

use tokio::fs;

pub struct Arg {
    pub dir: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Ret {
    pub dirs: Option<Vec<PathBuf>>,
    pub files: Option<Vec<PathBuf>>,
}

use regex::Regex;

pub async fn run(arg: Arg) -> Result<Ret> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[0-9a-f]{2}\.json$").unwrap();
    }
    let mut dirs = Vec::<PathBuf>::new();
    let mut files = Vec::<PathBuf>::new();
    let mut read_dir = fs::read_dir(arg.dir).await?;
    loop {
        let entry = read_dir.next_entry().await?;
        if let Some(entry) = entry {
            let path = entry.path();
            let metadata = fs::metadata(&path).await?;
            if metadata.is_dir() {
                dirs.push(path)
            } else if metadata.is_file() && RE.is_match(&path.file_name().unwrap().to_string_lossy()) {
                files.push(path)
            }
        } else {
            break;
        }
    }
    Ok(Ret {
        dirs: if dirs.len() == 0 { None } else { Some(dirs) },
        files: if files.len() == 0 { None } else { Some(files) },
    })
}

