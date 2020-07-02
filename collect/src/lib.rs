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

#[derive(Serialize, Deserialize)]
pub struct Ret {
    pub records: Vec<cards::Record>
}

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

use serde::{
    Serialize, 
    Deserialize,
};

const CALLBACK_THROTTLE: u128 = 100; //ms
pub async fn cards<'a, Cb>(
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
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

    let mut records = Vec::<cards::Record>::new();
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
                                if let cards::Card::Record(record) = ret {
                                    records.push(record)
                                }

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

    Ok(Ret{records})
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

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    use term::Term;

    #[tokio::test]
    async fn test_collect() -> Result<()> {
        init();

        let arg = Arg { 
            out_dir: Path::new("../out"),
            thread_limit_file: 3,
        };

        let mut term = Term::init(term::Arg::new().header("Чтение карточек . . ."))?;
        let start = Instant::now();
        let ret = cards(arg, Some(|arg: CallbackArg| -> Result<()> {
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
        println!("{}, Карточки прочитаны: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.records.len());

        let file_path = Path::new("out_test/some.csv");
        let arg = to_csv::Arg {
            records: &ret.records,
            file_path: &file_path,
        };
        to_csv::write(arg).await?;

        // if let Some(dir_path) = file_path.parent() {
        //     fs::create_dir_all(dir_path).await?;
        // }
        // let mut wtr = csv::Writer::from_path("out_test/some.csv")?;
        //
        // for record in ret.records {
        //     wtr.serialize(record)?;
        // }
        // wtr.flush()?;
        println!("{}, Записаны в файл {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), file_path);

        Ok(())
    }
}

