// use diaps;
use env_logger;
use http::StatusCode;
use anyhow::{bail, 
    // ensure, Context, 
    Result, 
    // anyhow,
};
use log::{info};
use std::time::Instant;
use serde_json::{json};
use std::collections::HashMap;

const COUNT_LIMIT: u64 = 4900;
const PRICE_PRECISION: isize = 20000;
const PRICE_MAX_INC: isize = 1000000;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let count_limit: u64 = diaps::param("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
    let price_precision: isize = diaps::param("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
    let price_max_inc: isize = diaps::param("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;
    //
    // let now = Instant::now();
    // let response = reqwest::get("http://auth:3000").await?;
    // let key = match response.status() {
    //     StatusCode::OK => {
    //         response.text().await?
    //     },
    //     StatusCode::NOT_FOUND => {
    //         bail!("auth: NOT_FOUND: {}", response.text().await?);
    //     },
    //     StatusCode::INTERNAL_SERVER_ERROR => {
    //         bail!("auth: INTERNAL_SERVER_ERROR: {}", response.text().await?);
    //     },
    //     _ => {
    //         unreachable!();
    //     },
    // };
    // info!("({} ms) key!: {}", 
    //     Instant::now().duration_since(now).as_millis(), 
    //     key,
    // );
    //
    // let now = Instant::now();
    let params = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
    //
    // let diaps::GetRet {last_stamp, diaps, checks_total} = diaps::get( diaps::GetArg { 
    //     key: &key, 
    //     params, 
    //     count_limit, 
    //     price_precision, 
    //     price_max_inc,
    // }).await?;
    //
    // info!("({} ms) last_stamp: {}, diaps: {:?}, len: {:?}, total_checks: {:?}", 
    //     Instant::now().duration_since(now).as_millis(),
    //     last_stamp, 
    //     diaps, 
    //     diaps.len(), 
    //     checks_total,
    // );
    //
    //

    let mut diaps: Vec<diaps::Diap> = Vec::new();
    diaps.push(diaps::Diap { price_min: None, price_max: Some(234376), count: 4761, checks: 7});
    diaps.push(diaps::Diap { price_min: Some(234377), price_max: Some(379514), count: 4864, checks: 9});
    diaps.push(diaps::Diap { price_min: Some(520616), price_max: Some(739205), count: 4708, checks: 8 });
    diaps.push(diaps::Diap { price_min: Some(739206), price_max: Some(1200685), count: 4829, checks: 12 });
    diaps.push(diaps::Diap { price_min: Some(1200685), price_max: Some(2200686), count: 3355, checks: 2 });
    diaps.push(diaps::Diap { price_min: Some(2200687), price_max: None, count: 1735, checks: 1 });
    let mut diap_store: HashMap<String, DiapStoreItem> = HashMap::new();
    let checks_total = 48;
    let last_stamp = 0;
    let key = format!("{}:{}:{}:{}", params, count_limit, price_precision, price_max_inc);
    diap_store.insert(key, DiapStoreItem {
        timestamp: chrono::Utc::now(),
        diaps,
        checks_total,
        last_stamp,
    });
    println!("diaps: {}", serde_json::to_string_pretty(&diap_store).unwrap());
    // let json = json!(
    //     {"categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private:4900:20000:1000000": {
    //         "timestamp": "2020-06-25T13:12:00Z",
    //         "diaps": [
    //             // {"price_min": None, "price_max": Some(234376), "count": 4761, "checks": 7 },
    //             {"price_max": Some(234376), "count": 4761, "checks": 7 },
    //             {"price_min": Some(234377), "price_max": Some(379514), "count": 4864, "checks": 9 },
    //             {"price_min": Some(379515), "price_max": Some(520615), "count": 4691, "checks": 9 },
    //             {"price_min": Some(520616), "price_max": Some(739205), "count": 4708, "checks": 8 },
    //             {"price_min": Some(739206), "price_max": Some(1200685), "count": 4829, "checks": 12 },
    //             {"price_min": Some(1200685), "price_max": Some(2200686), "count": 3355, "checks": 2 },
    //             // {"price_min": Some(2200687), "price_max": None, "count": 1735, "checks": 1 },
    //             {"price_min": Some(2200687), "count": 1735, "checks": 1 },
    //         ],
    //     }}
    // );
    // println!("json: {}", serde_json::to_string_pretty(&json).unwrap());

        // let file_path = &self.file_path;
        // match file_path.parent() {
        //     None => Err("could not extract dir_path".to_owned()),
        //     Some(dir_path) => {
        //         if !dir_path.is_dir() {
        //             if let Err(err) = fs::create_dir_all(dir_path).await {
        //                 return Err(err.to_string());
        //             }
        //         }
        //         // let dir_path = self.dir_path();
        //         // if !dir_path.is_dir() {
        //         //     if let Err(err) = fs::create_dir_all(&dir_path).await {
        //         //         return Err(format!("create_dir_all {:?}: {}", dir_path, err));
        //         //     }
        //         // }
        //         // let file_path = self.file_path();
        //         let Self { content, .. } = self;
        //         let mut encoder = GzEncoder::new(Vec::new(), Compression::new(2));
        //         if let Err(err) = encoder.write_all(content.as_bytes()) {
        //             Err(format!(
        //                 "{}::to_file {:?}: {}",
        //                 stringify!(BunchRaw),
        //                 &file_path,
        //                 err
        //             ))
        //         } else {
        //             match encoder.finish() {
        //                 Err(err) => Err(format!(
        //                     "{}::to_file {:?}: {}",
        //                     stringify!(BunchRaw),
        //                     &file_path,
        //                     err
        //                 )),
        //                 Ok(compressed_bytes) => {
        //                     match fs::write(&file_path, compressed_bytes).await {
        //                         Err(err) => Err(format!(
        //                             "{}::to_file {:?}: {}",
        //                             stringify!(BunchRaw),
        //                             &file_path,
        //                             err
        //                         )),
        //                         Ok(()) => {
        //                             // info!("written {:?}", file_path);
        //                             Ok(())
        //                         }
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }
    Ok(())
}

// #[macro_use]
// extern crate serde;

// #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
// #[derive(Serialize, Deserialize)]
use serde::ser::{Serialize, Serializer, SerializeStruct};
pub struct DiapStoreItem {
    timestamp: chrono::DateTime<chrono::Utc>,
    last_stamp: usize,
    checks_total: usize, 
    diaps: Vec<diaps::Diap>,
}

impl Serialize for DiapStoreItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 3 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("Color", 3)?;
        state.serialize_field("timestamp", &self.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))?;
        state.serialize_field("last_stamp", &self.last_stamp)?;
        state.serialize_field("checks_total", &self.checks_total)?;
        state.serialize_field("diaps", &self.diaps)?;
        state.end()
    }
}


