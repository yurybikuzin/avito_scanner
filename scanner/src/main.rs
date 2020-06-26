use env_logger;
use http::StatusCode;
use anyhow::{bail, Result};
use log::{info};
use std::time::Instant;
use diap_store::{DiapStore};
use std::path::Path;

const COUNT_LIMIT: u64 = 4900;
const PRICE_PRECISION: isize = 20000;
const PRICE_MAX_INC: isize = 1000000;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let count_limit: u64 = diaps::param("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
    let price_precision: isize = diaps::param("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
    let price_max_inc: isize = diaps::param("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;
    let params = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";

    let diaps_store_file_spec= &Path::new("out/diaps.json");

    let get_arg = diaps::GetArg {
        params,
        count_limit,
        price_precision,
        price_max_inc,
    };

    let diap_store = DiapStore::from_file(diaps_store_file_spec).await;
    let mut diap_store = match diap_store {
        Ok(diap_store) => diap_store, 
        Err(_) => DiapStore::new(),
    };
    let fresh_duration = chrono::Duration::minutes(30);
    let mut key: Option<String> = None;
    let diaps_ret = match diap_store.get_diaps(&get_arg, fresh_duration) {
        Some(item) => &item.ret,
        None => {
            let key_ret = get_key().await?;
            key = Some(key_ret);
            let get_ret = diaps::get(key.as_ref().unwrap(), &get_arg).await?;
            diap_store.set_diaps(&get_arg, get_ret);
            diap_store.to_file(diaps_store_file_spec).await?;
            &diap_store.get_diaps(&get_arg, fresh_duration).unwrap().ret
        },
    };
    println!("diaps_ret: {}", serde_json::to_string_pretty(&diaps_ret)?);
    Ok(())
}

async fn get_key () -> Result<String> {
    let now = Instant::now();
    let response = reqwest::get("http://auth:3000").await?;
    let key = match response.status() {
        StatusCode::OK => {
            response.text().await?
        },
        StatusCode::NOT_FOUND => {
            bail!("auth: NOT_FOUND: {}", response.text().await?);
        },
        StatusCode::INTERNAL_SERVER_ERROR => {
            bail!("auth: INTERNAL_SERVER_ERROR: {}", response.text().await?);
        },
        _ => {
            unreachable!();
        },
    };
    info!("({} ms) key!: {}", 
        Instant::now().duration_since(now).as_millis(), 
        key,
    );
    Ok(key)
}

