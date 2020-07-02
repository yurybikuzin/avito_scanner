#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use env_logger;
use diap_store::{DiapStore};
use id_store::{IdStore};
use std::path::Path;
use std::time::Instant;

// ============================================================================
// ============================================================================

const DIAP_STORE_FILE_SPEC: &str = "out/diaps.json";
const DIAP_FRESH_DURATION: usize = 1440; //30; // minutes
const COUNT_LIMIT: u64 = 4900;
const PRICE_PRECISION: isize = 20000;
const PRICE_MAX_INC: isize = 1000000;

const ID_STORE_FILE_SPEC: &str = "out/ids.json";
const ID_FRESH_DURATION: usize = 1440; //30; // minutes
const PARAMS: &str = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
const THREAD_LIMIT_NETWORK: usize = 1;
const THREAD_LIMIT_FILE: usize = 12;
const ITEMS_PER_PAGE: usize = 50;
const RETRY_COUNT: usize = 3;

const CARD_OUT_DIR_SPEC: &str = "out";

use term::Term;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let params = env::get("AVITO_PARAMS", PARAMS.to_owned())?;
    let id_store_file_spec= &Path::new(ID_STORE_FILE_SPEC);
    let id_fresh_duration: usize = env::get("AVITO_ID_FRESH_DURATION", ID_FRESH_DURATION)?;
    let id_fresh_duration = chrono::Duration::minutes(id_fresh_duration as i64);

    let id_store = IdStore::from_file(id_store_file_spec).await;
    let mut id_store = match id_store {
        Ok(id_store) => id_store, 
        Err(_) => IdStore::new(),
    };

    let thread_limit_network = env::get("AVITO_THREAD_LIMIT_NETWORK", THREAD_LIMIT_NETWORK)?;
    let thread_limit_file = env::get("AVITO_THREAD_LIMIT_FILE", THREAD_LIMIT_FILE)?;
    let retry_count = env::get("AVITO_RETRY_COUNT", RETRY_COUNT)?;

    let mut auth = auth::Lazy::new(Some(auth::Arg::new()));

    let ids = match id_store.get_ids(&params, id_fresh_duration) {
        Some(item) => {
            println!("{} ids loaded from {:?}", item.ret.len(), id_store_file_spec.to_string_lossy());
            &item.ret
        },
        None => {

            let count_limit: u64 = env::get("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
            let price_precision: isize = env::get("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
            let price_max_inc: isize = env::get("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;
            let diap_fresh_duration: usize = env::get("AVITO_DIAP_FRESH_DURATION", DIAP_FRESH_DURATION)?;
            let diap_fresh_duration = chrono::Duration::minutes(diap_fresh_duration as i64);

            let diap_store_file_spec= &Path::new(DIAP_STORE_FILE_SPEC);

            let diaps_arg = diaps::Arg {
                params: &params,
                count_limit,
                price_precision,
                price_max_inc,
            };

            let diap_store = DiapStore::from_file(diap_store_file_spec).await;
            let mut diap_store = match diap_store {
                Ok(diap_store) => diap_store, 
                Err(_) => DiapStore::new(),
            };
            let diaps_ret = match diap_store.get_diaps(&diaps_arg, diap_fresh_duration) {
                Some(item) => &item.ret,
                None => {
                    let mut term = Term::init(term::Arg::new().header("Определение диапазонов цен . . ."))?;
                    let start = Instant::now();
                    let diaps_ret = diaps::get(
                        &mut auth,
                        diaps_arg.clone(), 
                        Some(|arg: diaps::CallbackArg| -> Result<()> {
                            term.output(format!("count_total: {}, checks_total: {}, elapsed_millis: {}, per_millis: {}, detected_diaps: {}, price: {}..{}/{}, count: {}", 
                                arg.count_total,
                                arg.checks_total,
                                arrange_millis::get(arg.elapsed_millis),
                                arrange_millis::get(arg.per_millis),
                                arg.diaps_detected,
                                if arg.price_min.is_none() { "".to_owned() } else { arg.price_min.unwrap().to_string() },
                                if arg.price_max.is_none() { "".to_owned() } else { arg.price_max.unwrap().to_string() },
                                if arg.price_max_delta.is_none() { "".to_owned() } else { arg.price_max_delta.unwrap().to_string() },
                                if arg.count.is_none() { "".to_owned() } else { arg.count.unwrap().to_string() },
                            ))
                        })
                    ).await?;
                    let diaps_len = diaps_ret.diaps.len();
                    diap_store.set_diaps(&diaps_arg, diaps_ret);
                    diap_store.to_file(diap_store_file_spec).await?;
                    println!("{}, Определены диапазоны ({}) цен и записаны в {:?} ", arrange_millis::get(Instant::now().duration_since(start).as_millis()), diaps_len, diap_store_file_spec.to_string_lossy());
                    &diap_store.get_diaps(&diaps_arg, diap_fresh_duration).unwrap().ret
                },
            };

            let items_per_page = env::get("AVITO_ITEMS_PER_PAGE", ITEMS_PER_PAGE)?;

            let ids_arg = ids::Arg {
                params: &params,
                diaps_ret: &diaps_ret, 
                thread_limit_network,
                items_per_page,
                retry_count,
            };
            let mut term = Term::init(term::Arg::new().header("Получение списка идентификаторов . . ."))?;
            let start = Instant::now();
            let ids_ret = ids::get(&mut auth, ids_arg, Some(|arg: ids::CallbackArg| -> Result<()> {
                term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}, ids_len: {}", 
                    arrange_millis::get(arg.elapsed_millis), 
                    arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                    arrange_millis::get(arg.remained_millis), 
                    arrange_millis::get(arg.per_millis), 
                    arg.elapsed_qt,
                    arg.elapsed_qt + arg.remained_qt,
                    arg.remained_qt,
                    arg.ids_len,
                ))
            })).await?;
            let ids_len = ids_ret.len();
            id_store.set_ids(&params, ids_ret);
            id_store.to_file(id_store_file_spec).await?;

            println!("{}, Список идентификаторов ({}) получен и записан в {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ids_len, id_store_file_spec.to_string_lossy());

            &id_store.get_ids(&params, id_fresh_duration).unwrap().ret
        },
    };

    let out_dir = &Path::new(CARD_OUT_DIR_SPEC);

    let mut term = Term::init(term::Arg::new().header("Получение объявлений . . ."))?;
    let arg = cards::Arg {
        ids: &ids, 
        out_dir,
        thread_limit_network,
        thread_limit_file,
        retry_count,
    };
    let start = Instant::now();
    let ret = cards::fetch_and_save(&mut auth, arg, Some(|arg: cards::CallbackArg| -> Result<()> {
        term.output(format!("time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
            arrange_millis::get(arg.elapsed_millis), 
            arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
            arrange_millis::get(arg.remained_millis), 
            arrange_millis::get(arg.per_millis), 
            arg.elapsed_qt,
            arg.elapsed_qt + arg.remained_qt,
            arg.remained_qt,
        ))
    })).await?;
    println!("{}, Объявления получены: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.received_qt);

    let arg = collect::Arg { 
        out_dir: Path::new("out"),
        thread_limit_file: 3,
    };

    let mut term = Term::init(term::Arg::new().header("Чтение карточек . . ."))?;
    let start = Instant::now();
    let ret = collect::cards(arg, Some(|arg: collect::CallbackArg| -> Result<()> {
        match arg {
            collect::CallbackArg::ReadDir {elapsed_millis, dir_qt, file_qt} => {
                term.output(format!("time: {}, dirs: {}, files: {}", 
                    arrange_millis::get(elapsed_millis), 
                    dir_qt,
                    file_qt,
                ))
            },
            collect::CallbackArg::ReadFile {elapsed_millis, remained_millis, per100_millis, elapsed_qt, remained_qt} => {
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

    let start = Instant::now();
    let file_path = Path::new("out/records.csv");
    let arg = to_csv::Arg {
        file_path: &file_path,
        records: &ret.records,
    };
    to_csv::write(arg).await?;
    println!("{}, Записаны в файл {:?}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), file_path);

    Ok(())
}
