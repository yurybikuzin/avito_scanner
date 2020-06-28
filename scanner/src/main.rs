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
const THREAD_LIMIT: usize = 1;
const ITEMS_PER_PAGE: usize = 50;
const RETRY_COUNT: usize = 3;

const CARD_OUT_DIR_SPEC: &str = "out";
// use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // // let re = regex::Regex::new(r"My Public IPv4 is: <[^>]+>([^<]+)").unwrap();
    // let re = regex::Regex::new(r#""ipv4":\{"value":"([^"]+)"#).unwrap();
    //
    // let url = format!("https://yandex.ru/internet/");
    // let url = Url::parse(&url)?;
    // // let client = reqwest::Client::new();
    // //
    // // let proxy_http_url = env::get("PROXY_HTTP_URL")?;
    // // let proxy_https_url = env::get("PROXY_HTTPS_URL")?;
    // let client = reqwest::Client::builder()
    //     // .proxy(reqwest::Proxy::http("http://91.235.33.79")?)
    //     .proxy(reqwest::Proxy::all("https://91.235.33.91")?)
    //     // .proxy(reqwest::Proxy::https("https://169.57.1.85")?)
    //     .build()?
    // ;
    // let text = client.get(url).send().await?.text().await?;
    // match re.captures(&text) {
    //     None => info!("ip not found"),
    //     Some(cap) => info!("ip: {}", &cap[1]),
    // }

    

    let params = env::get("AVITO_PARAMS", PARAMS.to_owned())?;
    let id_store_file_spec= &Path::new(ID_STORE_FILE_SPEC);
    let id_fresh_duration: usize = env::get("AVITO_ID_FRESH_DURATION", ID_FRESH_DURATION)?;
    let id_fresh_duration = chrono::Duration::minutes(id_fresh_duration as i64);

    let id_store = IdStore::from_file(id_store_file_spec).await;
    let mut id_store = match id_store {
        Ok(id_store) => id_store, 
        Err(_) => IdStore::new(),
    };

    let thread_limit = env::get("AVITO_THREAD_LIMIT", THREAD_LIMIT)?;
    let retry_count = env::get("AVITO_RETRY_COUNT", RETRY_COUNT)?;

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
            let mut auth: Option<String> = None;
            let diaps_ret = match diap_store.get_diaps(&diaps_arg, diap_fresh_duration) {
                Some(item) => &item.ret,
                None => {
                    auth = Some(auth.unwrap_or(auth::get().await?));
                    let diaps_ret = diaps::get(auth.as_ref().unwrap(), &diaps_arg).await?;
                    diap_store.set_diaps(&diaps_arg, diaps_ret);
                    diap_store.to_file(diap_store_file_spec).await?;
                    &diap_store.get_diaps(&diaps_arg, diap_fresh_duration).unwrap().ret
                },
            };
            auth = Some(auth.unwrap_or(auth::get().await?));

            let items_per_page = env::get("AVITO_ITEMS_PER_PAGE", ITEMS_PER_PAGE)?;

            let ids_arg = ids::Arg {
                auth: auth.as_ref().unwrap(),
                params: &params,
                diaps_ret: &diaps_ret, 
                thread_limit,
                items_per_page,
                retry_count,
            };
            println!("ids::get");
            let now = Instant::now();
            let ids_ret = ids::get(ids_arg, &|arg: ids::CallbackArg| {
                println!("{}ids::get: time: {}/{}-{}, per: {}, qt: {}/{}-{}, ids_len: {}", 
                    ansi_escapes::EraseLines(2),
                    arrange_millis::get(arg.elapsed_millis), 
                    arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                    arrange_millis::get(arg.remained_millis), 
                    arrange_millis::get(arg.per_millis), 
                    arg.elapsed_qt,
                    arg.elapsed_qt + arg.remained_qt,
                    arg.remained_qt,
                    arg.ids_len,
                );
            }).await?;
            let ids_len = ids_ret.len();
            println!("{} ids fetched, took {}", ids_len, arrange_millis::get(Instant::now().duration_since(now).as_millis()));
            println!("{}", ansi_escapes::EraseLines(2));

            id_store.set_ids(&params, ids_ret);
            id_store.to_file(id_store_file_spec).await?;

            println!("{} ids written to {:?}", ids_len, id_store_file_spec.to_string_lossy());
            &id_store.get_ids(&params, id_fresh_duration).unwrap().ret
        },
    };


    let out_dir_spec = &Path::new(CARD_OUT_DIR_SPEC);
    cards::fetch_and_save(cards::Arg {
        auth: auth.as_ref().unwrap(),
        ids, 
        out_dir_spec,
        thread_limit,
        retry_count,
    }).await?;

    Ok(())
}
