#![recursion_limit="1024"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::time::Instant;
use std::path::Path;

#[macro_use] extern crate lazy_static;

use futures::{
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};

mod read_dir;
mod read_file;

// ============================================================================
// ============================================================================

pub struct Arg<'a> {
    pub out_dir: &'a Path, 
    pub thread_limit_file: usize,
}

macro_rules! push_fut_readdir {
    ($fut_queue: expr, $vec_dir: expr, $i_dir: expr, $used_file_threads: expr) => {
        let dir = $vec_dir[$i_dir].as_path();
        let arg = OpArg::ReadDir (read_dir::Arg {
            dir: dir.to_owned(),
        });
        let fut = op(arg);
        $fut_queue.push(fut);
        $i_dir += 1;
        $used_file_threads += 1;
    };
}

macro_rules! push_fut_readfile {
    ($fut_queue: expr, $vec_file: expr, $i_file: expr, $used_file_threads: expr) => {
        let file_path = $vec_file[$i_file].as_path();
        let arg = OpArg::ReadFile (read_file::Arg {
            file_path: file_path.to_owned(),
        });
        let fut = op(arg);
        $fut_queue.push(fut);
        $i_file += 1;
        $used_file_threads += 1;
    };
}

pub enum CallbackArg {
    ReadDir {
        elapsed_millis: u128,
        dir_qt: usize,
        file_qt: usize,
    },
    ReadFile {
        elapsed_qt: usize,
        remained_qt: usize,
        elapsed_millis: u128,
        remained_millis: u128,
        per100_millis: u128,
    }
}

pub trait Ret {
    fn adopt_file(&mut self, file_path: PathBuf, fetched: cards::Fetched) -> Result<()>;
}

// pub struct Records (pub Vec::<cards::Record>);
pub struct ErrorCard {
    pub file_path: PathBuf,
    pub json: serde_json::Value,
    pub error: String,
}

pub struct Card {
    pub file_path: PathBuf,
    pub record: cards::Record,
}

pub struct Records {
    pub cards: Vec::<Card>,
    pub errors: Vec::<ErrorCard>,
    pub not_found: Vec::<PathBuf>,
    pub no_text: Vec::<PathBuf>,
}

impl Records {
    pub fn new() -> Self {
        Self {
            cards: Vec::new(),
            errors: Vec::new(),
            not_found: Vec::new(),
            no_text: Vec::new(),
        }
        // Self(Vec::new())
    }
}

impl Ret for Records {
    fn adopt_file(&mut self, file_path: PathBuf, fetched: cards::Fetched) -> Result<()> {
        match fetched {
            cards::Fetched::WithError {json, error} => {
                self.errors.push(ErrorCard{file_path, json, error})
            },
            cards::Fetched::Record(record) => {
                self.cards.push(Card {file_path, record});
            },
            cards::Fetched::NotFound => {
                self.not_found.push(file_path);
            },
            cards::Fetched::NoText => {
                self.no_text.push(file_path);
            },
        }
        // self.0.push(record);
        Ok(())
    }
}

// use std::collections::HashSet;
// pub struct Autocatalog (pub HashSet<String>);
// impl Autocatalog {
//     pub fn new() -> Self {
//         Self(HashSet::new())
//     }
// }

// use regex::Regex;
// impl Ret for Autocatalog {
//     fn adopt_record(&mut self, record: cards::Record) -> Result<()> {
//         if let Some(url) = record.autocatalog_url {
//             // lazy_static! {
//             //     static ref RE: Regex = Regex::new(r"/\d+$").unwrap();
//             // }
//             // let url = RE.replace(&url, "").to_string();
//             self.0.insert(url);
//         }
//         Ok(())
//     }
// }

use std::path::PathBuf;

macro_rules! callback_dir {
    ($callback: expr, $start: expr, $vec_dir: expr, $vec_file: expr) => {
        let elapsed_millis = Instant::now().duration_since($start).as_millis(); 
        $callback(CallbackArg::ReadDir {
            elapsed_millis, 
            dir_qt: $vec_dir.len(),
            file_qt: $vec_file.len(),
        })?;
    };
}

macro_rules! callback_file {
    ($callback: expr, $start: expr, $elapsed_qt: expr, $remained_qt: expr) => {
        let elapsed_micros = Instant::now().duration_since($start).as_micros(); 
        let per_micros = elapsed_micros / $elapsed_qt as u128;
        let remained_micros = per_micros * $remained_qt as u128;
        $callback(CallbackArg::ReadFile {
            elapsed_qt: $elapsed_qt,
            remained_qt: $remained_qt,
            elapsed_millis: elapsed_micros / 1000, 
            remained_millis: remained_micros / 1000, 
            per100_millis: per_micros / 100,
        })?;
    };
}

const CALLBACK_THROTTLE: u128 = 100; //ms

pub async fn items<'a, Cb, R>(
    arg: Arg<'a>, 
    items: &mut R,
    mut callback: Option<Cb>,
) -> Result<()>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
    R: Ret,
{
    let mut vec_dir: Vec<PathBuf> = vec![arg.out_dir.to_owned()];
    let mut i_dir = 0;

    let mut vec_file = Vec::<PathBuf>::new();
    let mut i_file: usize = 0;

    let mut used_file_threads = 0;

    let mut fut_queue = FuturesUnordered::new();
    push_fut_readdir!(fut_queue, vec_dir, i_dir, used_file_threads);

    let mut elapsed_qt: usize = 0;
    let mut remained_qt: usize = 0;
    let mut last_callback = Instant::now();
    let mut start_file: Option<Instant> = None; 
    let start_dir = Instant::now(); 

    loop {
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(err) => {
                        return Err(err.context("collect::cards"));
                    },
                    Ok(ret) => {
                        match ret {
                            OpRet::ReadFile(ret) => {
                                used_file_threads -= 1;
                                let read_file::Ret {file_path, fetched} = ret;
                                items.adopt_file(file_path, fetched)?;
                                // if let cards::Fetched::Record(record) = ret {
                                //     items.adopt_record(record)?;
                                // }

                                callback = if let Some(mut callback) = callback {
                                    elapsed_qt += 1;
                                    if remained_qt > 0 {
                                        remained_qt -= 1;
                                    }
                                    if let Some(start_file) = start_file {
                                        if elapsed_qt > 0 && Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                            callback_file!(callback, start_file, elapsed_qt, remained_qt);
                                            last_callback = Instant::now();
                                        }
                                    }
                                    Some(callback)
                                } else {
                                    None
                                };

                                while used_file_threads < arg.thread_limit_file && i_file < vec_file.len() {
                                    push_fut_readfile!(fut_queue, vec_file, i_file, used_file_threads);
                                }
                            },
                            OpRet::ReadDir(read_dir::Ret{dirs, files}) => {
                                used_file_threads -= 1;
                                if let Some(dirs) = dirs {
                                    for dir in dirs {
                                        vec_dir.push(dir);
                                    }
                                }
                                while used_file_threads < arg.thread_limit_file && i_dir < vec_dir.len() {
                                    push_fut_readdir!(fut_queue, vec_dir, i_dir, used_file_threads);
                                }
                                if let Some(files) = files {
                                    callback = if let Some(mut callback) = callback {
                                        remained_qt += files.len();
                                        if let Some(start_file) = start_file {
                                            if Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                                callback_file!(callback, start_file, elapsed_qt, remained_qt);
                                                last_callback = Instant::now();
                                            }
                                        }
                                        Some(callback)
                                    } else {
                                        None
                                    };

                                    for file in files {
                                        vec_file.push(file);
                                    }

                                    callback = if let Some(mut callback) = callback {
                                        if Instant::now().duration_since(last_callback).as_millis() > CALLBACK_THROTTLE {
                                            if elapsed_qt == 0 {
                                                callback_dir!(callback, start_dir, vec_dir, vec_file);
                                                last_callback = Instant::now();
                                            }
                                        }
                                        Some(callback)
                                    } else {
                                        None
                                    };
                                }

                                while used_file_threads < arg.thread_limit_file && i_file < vec_file.len() {
                                    if i_file == 0 {
                                        start_file = Some(Instant::now());
                                    }
                                    push_fut_readfile!(fut_queue, vec_file, i_file, used_file_threads);
                                }
                            },
                        }
                    },
                }
            },
            complete => {
                break;
            },
        }
    }

    if let Some(mut callback) = callback {
        if let Some(start_file) = start_file {
            if elapsed_qt > 0  {
                callback_file!(callback, start_file, elapsed_qt, remained_qt);
            }
        }
    }

    Ok(())
}

enum OpArg {
    ReadDir(read_dir::Arg),
    ReadFile(read_file::Arg),
}

enum OpRet {
    ReadDir(read_dir::Ret),
    ReadFile(read_file::Ret),
}

async fn op(arg: OpArg) -> Result<OpRet> {
    match arg {
        OpArg::ReadDir(arg) => {
            let ret = read_dir::run(arg).await?;
            Ok(OpRet::ReadDir(ret))
        },
        OpArg::ReadFile(arg) => {
            let ret = read_file::run(arg).await?;
            Ok(OpRet::ReadFile(ret))
        },
    }
}

    // use lazy_static::lazy_static;
use regex::Regex;
pub fn convert_file_path(file_path: PathBuf) -> Result<PathBuf> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
            ^
            (/out/)
            ([\da-f][\da-f])/
            ([\da-f][\da-f])/
            ([\da-f][\da-f])/
            ([\da-f][\da-f])/
            ([\da-f])([\da-f])/
            ([\da-f][\da-f])/
            ([\da-f][\da-f])/
            ([\da-f][\da-f]\.json)
            # $
        "#).unwrap();
    }
    if let Some(caps) = RE.captures(&file_path.to_string_lossy()) {
        let mut items = Vec::<&str>::new();
        items.push(caps.get(1).unwrap().as_str());
        items.push("cards/");
        items.push(caps.get(2).unwrap().as_str());
        items.push(caps.get(3).unwrap().as_str());
        items.push(caps.get(4).unwrap().as_str());
        items.push(caps.get(5).unwrap().as_str());
        items.push(caps.get(6).unwrap().as_str());
        items.push("/");
        items.push(caps.get(7).unwrap().as_str());
        items.push(caps.get(8).unwrap().as_str());
        items.push(caps.get(9).unwrap().as_str());
        items.push(caps.get(10).unwrap().as_str());
        let s = items.into_iter().collect::<String>();
        return Ok(PathBuf::from(s))
    } else {
        bail!("no match for {:?}", file_path);
    }
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;

    use term::Term;

    #[tokio::test]
    async fn test_convert_file_path() -> Result<()> {
        test_helper::init();

        let file_path = std::path::PathBuf::from("/out/00/00/00/00/25/3f/9e/88.json");
        let file_path = convert_file_path(file_path)?;
        assert_eq!(file_path.to_string_lossy(), "/out/cards/000000002/53f9e88.json");

        Ok(())
    }

    #[tokio::test]
    async fn test_convert() -> Result<()> {
        test_helper::init();

        let arg = Arg { 
            out_dir: Path::new("/out"),
            thread_limit_file: 6,
        };

        let mut term = Term::init(term::Arg::new().header("Чтение карточек . . ."))?;
        let start = Instant::now();
        let mut records = Records::new();
        items(arg, &mut records, Some(|arg: CallbackArg| -> Result<()> {
            match arg {
                CallbackArg::ReadDir {elapsed_millis, dir_qt, file_qt} => {
                    term.output(format!("time: {}, dirs: {}, files: {}", 
                        arrange_millis::get(elapsed_millis), 
                        dir_qt,
                        file_qt,
                    ))
                },
                CallbackArg::ReadFile {elapsed_millis, remained_millis, per100_millis, elapsed_qt, remained_qt} => {
                    term.output(format!("time: {}/{}-{}, per100: {}, qt: {}/{}-{}", 
                        arrange_millis::get(elapsed_millis), 
                        arrange_millis::get(elapsed_millis + remained_millis), 
                        arrange_millis::get(remained_millis), 
                        arrange_millis::get(per100_millis), 
                        elapsed_qt,
                        elapsed_qt + remained_qt,
                        remained_qt,
                    ))
                },
            }
        })).await?;
        info!("{}, Карточки прочитаны", arrange_millis::get(Instant::now().duration_since(start).as_millis()));
        info!("cards: {}, errors: {}, not_found: {}, no_text: {}", records.cards.len(), records.errors.len(), records.not_found.len(), records.no_text.len());

        let start = Instant::now();
        let mut errors = Vec::<ErrorCard>::new();
        for ErrorCard {file_path, error: _, json} in records.errors.into_iter() {
            let json = json::Json::new(json, json::JsonSource::FilePath(file_path.to_owned()));
            match cards::Fetched::parse_json(&json) {
                Ok(record) => {
                    records.cards.push(Card {file_path, record});
                },
                Err(err) => {
                    let error = err.to_string();
                    if !error.contains("year") {
                        bail!("err: {}", err);
                    }
                    errors.push(ErrorCard { file_path, json: json.value, error});
                },
            }
        }
        records.errors =  errors;
        info!("{}, cards: {}, errors: {}, not_found: {}, no_text: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), records.cards.len(), records.errors.len(), records.not_found.len(), records.no_text.len());


        // let mut term = Term::init(term::Arg::new().header("write cards . . ."))?;

        info!("write cards . . .");
        let start = std::time::Instant::now();
        for Card {file_path, record} in records.cards.into_iter() {
            let file_path = convert_file_path(file_path)?;
            if let Some(dir_path) = file_path.parent() {
                fs::create_dir_all(dir_path).await?;
            }
            let mut file = fs::File::create(file_path).await?;
            let fetched = cards::Fetched::Record(record);
            let json = serde_json::to_string_pretty(&fetched)?;
            file.write_all(json.as_bytes()).await?;
        }
        info!("{}, cards written", arrange_millis::get(Instant::now().duration_since(start).as_millis()));

        info!("write errors . . .");
        let start = std::time::Instant::now();
        for ErrorCard {file_path, error, json} in records.errors.into_iter() {
            let file_path = convert_file_path(file_path)?;
            if let Some(dir_path) = file_path.parent() {
                fs::create_dir_all(dir_path).await?;
            }
            let mut file = fs::File::create(file_path).await?;
            let fetched = cards::Fetched::WithError{error, json};
            let json = serde_json::to_string_pretty(&fetched)?;
            file.write_all(json.as_bytes()).await?;
        }
        info!("{}, errors written", arrange_millis::get(Instant::now().duration_since(start).as_millis()));

        info!("write not_found . . .");
        let start = std::time::Instant::now();
        for  file_path in records.not_found.into_iter() {
            let file_path = convert_file_path(file_path)?;
            if let Some(dir_path) = file_path.parent() {
                fs::create_dir_all(dir_path).await?;
            }
            let mut file = fs::File::create(file_path).await?;
            let fetched = cards::Fetched::NotFound;
            let json = serde_json::to_string_pretty(&fetched)?;
            file.write_all(json.as_bytes()).await?;
        }
        info!("{}, not_found written", arrange_millis::get(Instant::now().duration_since(start).as_millis()));

        Ok(())
    }

    use tokio::fs;
    use tokio::prelude::*;

}

