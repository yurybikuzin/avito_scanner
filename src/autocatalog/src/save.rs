
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path, PathBuf};
// use serde_json::{Value};

use tokio::fs::{self, File};
use tokio::prelude::*;
use url::Url;

use super::fetched::Fetched;

type Item = str;
pub struct Arg<I: AsRef<Item>, P: AsRef<Path>> {
    pub item: I,
    pub fetched: Fetched,
    // pub out_dir: &'a Path,
    pub out_dir: P,
}

pub struct Ret ();

pub async fn run<I: AsRef<Item>, P: AsRef<Path>>(arg: Arg<I, P>) -> Result<Ret> {
    info!("out_dir: {:?}", arg.out_dir.as_ref());

    match arg.fetched {
        Fetched::Records(vec_record) => {
            info!("vec_record: {:?}", vec_record.len());
            for record in vec_record {
                let s = format!("{}{}/{}", arg.out_dir.as_ref().to_string_lossy(), arg.item.as_ref(), record.id);
                let file_path = PathBuf::from(s);
                save_json(record, file_path).await?;

                // let file_path = file_path.with_extension("json");
                //
                // if let Some(dir_path) = file_path.parent() {
                //     fs::create_dir_all(dir_path).await?;
                // }
                //
                // let mut file = File::create(file_path).await?;
                // let json = serde_json::to_string_pretty(&record)?;
                // file.write_all(json.as_bytes()).await?;
            }
        },
        _ => {
            let s = format!("{}{}", arg.out_dir.as_ref().to_string_lossy(), arg.item.as_ref());
            let file_path = PathBuf::from(s);
            save_json(arg.fetched, file_path).await?;
            // let file_path = file_path.with_extension("json");
            //
            // if let Some(dir_path) = file_path.parent() {
            //     fs::create_dir_all(dir_path).await?;
            // }
            //
            // let mut file = File::create(file_path).await?;
            // let json = serde_json::to_string_pretty(&arg.fetched)?;
            // file.write_all(json.as_bytes()).await?;
        }
    }

    Ok(Ret())
}

use serde::{Serialize};
async fn save_json<S: Serialize, P: AsRef<Path>>(json: S, file_path: P) -> Result<()> {
    let file_path = file_path.as_ref().with_extension("json");

    if let Some(dir_path) = file_path.parent() {
        fs::create_dir_all(dir_path).await?;
    }

    let mut file = File::create(file_path).await?;
    let json = serde_json::to_string_pretty(&json)?;
    file.write_all(json.as_bytes()).await?;

    Ok(())
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
        INIT.call_once(|| pretty_env_logger::init());
    }

    // use term::Term;
    // use json::{Json};
    // // use itertools::Itertools;
    // use std::collections::HashSet;
    // use std::iter::FromIterator;

    #[tokio::test]
    async fn test_save() -> Result<()> {
        init();

        let mut file = File::open("test_data/page.html").await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let text = std::str::from_utf8(&contents)?;
        let url = Url::parse("https://avito.ru/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896")?;
        let fetched = Fetched::parse(text, url)?;

        let arg = Arg {
            item: "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan",
            out_dir: Path::new("out_test"),
            fetched,
        };
        let _ret = run(arg).await?;

        // let items = Json::from_file("test_data/autocatalog_urls.json").await?;

        // let items = 
        //     items.iter_vec()?
        //     .map(|val| val.as_string().unwrap())
        //     .collect::<Vec<String>>()
        // ;
        // let items: HashSet<String> = HashSet::from_iter(items.iter().cloned());
        // let out_dir = Path::new("out_test");
        // let arg = Arg {
        //     items_to_check: &items,
        //     out_dir,
        //     thread_limit_network: 1,
        //     thread_limit_file: 12,
        //     retry_count: 3,
        // };
        //
        // let mut term = Term::init(term::Arg::new().header("Получение автокаталога . . ."))?;
        // let start = Instant::now();
        // let ret = fetch_and_save(arg, Some(|arg: CallbackArg| -> Result<()> {
        //     term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
        //         arrange_millis::get(arg.elapsed_millis), 
        //         arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
        //         arrange_millis::get(arg.remained_millis), 
        //         arrange_millis::get(arg.per_millis), 
        //         arg.elapsed_qt,
        //         arg.elapsed_qt + arg.remained_qt,
        //         arg.remained_qt,
        //     ))
        // })).await?;
        // println!("{}, Автокаталог получен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.received_qt);

        Ok(())
    }
}



