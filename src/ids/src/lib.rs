#![recursion_limit="256"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::time::Instant;
use std::collections::HashSet;

use url::Url;
use serde_json::Value;

use futures::{
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};

use client::{Client};
// https://m.avito.ru/api/9/items?key=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir&locationId=107620&params[110000]=329202&owner[]=private&sort=default&withImagesOnly=false&lastStamp=1595587140&display=list&page=1&limit=50&priceMax=393073
// https://m.avito.ru/api/9/items?key=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir&params[110000]=329230&categoryId=9&locationId=637640&privateOnly=1&searchRadius=100&sort=default&owner[]=private&page=3&lastStamp=1595584320&display=list&limit=30&pageId=H4sIAAAAAAAAA0u0MrSqLrYyNLRSKskvScyJT8svzUtRss60MjYwNbKuBQAkWBn0IAAAAA

// ============================================================================
// ============================================================================

pub struct Arg<'a> {
    pub params: &'a str,
    pub diaps_ret: &'a diaps::Ret, 
    pub items_per_page: usize,
    pub thread_limit_network: usize,
    pub client_provider: client::Provider,
}

macro_rules! push_fut {
    ($fut_queue: expr, $client: expr, $auth: expr, $arg: expr, $page: expr, $diap: expr) => {
        let fetch_arg = FetchArg {
            client: $client,
            auth: $auth.key().await?,
            diap: $diap, 
            page: $page,
            params: $arg.params.to_owned(),
            last_stamp: $arg.diaps_ret.last_stamp,
            items_per_page: $arg.items_per_page,
            // retry_count: $arg.retry_count,
        };
        let fut = fetch(fetch_arg);
        $fut_queue.push(fut);
    };
}

pub struct CallbackArg {
    pub elapsed_qt: u64,
    pub remained_qt: u64,
    pub elapsed_millis: u128,
    pub remained_millis: u128,
    pub per_millis: u128,
    pub ids_len: usize,
}

pub type Ret = HashSet<u64>;

pub async fn get<'a, Cb>(
    auth: &mut auth::Lazy,
    arg: Arg<'a>, 
    mut callback: Option<Cb>,
) -> Result<Ret>
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
 {
    let start = Instant::now();

    let mut diap_i = 0;
    let diaps_len = arg.diaps_ret.diaps.len();

    let mut fut_queue = FuturesUnordered::new();
    while diap_i < arg.thread_limit_network && diap_i < diaps_len {
        let client = arg.client_provider.build().await?;
        let diap = arg.diaps_ret.diaps[diap_i].clone();
        push_fut!(fut_queue, client, auth, arg, 1, diap);
        diap_i += 1;
    }
    let mut elapsed_qt: u64 = 0;
    let mut remained_qt = arg.diaps_ret.diaps.iter()
        .map(|diap| 
            (diap.count + arg.items_per_page as u64 - 1) / arg.items_per_page as u64
        )
        .sum::<u64>();

    let mut ids: Ret = HashSet::new(); 
    loop {
        select! {
            ret = fut_queue.select_next_some() => {
                match ret {
                    Err(err) => {
                        bail!("error: {}", err);
                    },
                    Ok(ret) => {
                        for id in ret.ids {
                            if ids.contains(&id) {
                                trace!("id '{}' already in set", id);
                            } else {
                                ids.insert(id);
                            }
                        }

                        callback = if let Some(mut callback) = callback {
                            elapsed_qt += 1;
                            if remained_qt > 0 {
                                remained_qt -= 1;
                            }
                            let elapsed_millis = Instant::now().duration_since(start).as_millis(); 
                            let per_millis = elapsed_millis / elapsed_qt as u128;
                            let remained_millis = per_millis * remained_qt as u128;
                            callback(CallbackArg {
                                elapsed_qt,
                                remained_qt,
                                elapsed_millis, 
                                remained_millis, 
                                per_millis,
                                ids_len: ids.len()
                            })?;
                            Some(callback)
                        } else {
                            None
                        };

                        if ret.arg.page < ret.page_qt {
                            push_fut!(fut_queue, ret.arg.client, auth, arg, ret.arg.page + 1, ret.arg.diap);
                        } else if diap_i < diaps_len {
                            push_fut!(fut_queue, ret.arg.client, auth, arg, 1, arg.diaps_ret.diaps[diap_i].clone());
                            diap_i += 1;
                        }
                    },
                }
            },
            complete => {
                break;
            },
        }
    }
    info!("{} ms, ids.len(): {}", 
        Instant::now().duration_since(start).as_millis(), 
        ids.len(),
    );
    
    Ok(ids)
}

// ============================================================================
// ============================================================================

struct FetchArg {
    // client: reqwest::Client,
    client: Client,
    auth: String,

    diap: diaps::Diap,
    page: usize,

    params: String,
    last_stamp: u64,
    items_per_page: usize,
    // retry_count: usize,
}

struct FetchRet {
    arg: FetchArg,
    ids: Ret,
    page_qt: usize,
}

async fn fetch(arg: FetchArg) -> Result<FetchRet>{
    let url = &format!("https://m.avito.ru/api/9/items?key={}&{}&lastStamp={}&display=list&page={}&limit={}{}{}", 
        arg.auth, 
        arg.params, 
        arg.last_stamp,
        arg.page,
        arg.items_per_page,
        if let Some(price_min) = arg.diap.price_min { format!("&priceMin={}", price_min) } else { "".to_owned() },
        if let Some(price_max) = arg.diap.price_max { format!("&priceMax={}", price_max) } else { "".to_owned() },
    );
    let url = Url::parse(&url)?;

    let now = Instant::now();
    let (text, status) = arg.client.get_text_status(url.clone()).await?;
    if status != http::StatusCode::OK {
        bail!("url: {}, status: {}", url, status);
    }
    trace!("{} ms , ids::fetch({:?})", Instant::now().duration_since(now).as_millis(), url.as_str());

    let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("failed to parse json from: {}", text))?;

    let count = json.get("result")
        .and_then(|val| val.get("count"))
        .ok_or(anyhow!("could not obtain result.count from '{}': {}", url, serde_json::to_string_pretty(&json)?))?
    ;
    let count = match count {
        Value::Number(number) => number.as_u64().ok_or(anyhow!("result.count expected to be u64, not {}", number))? as usize,
        val @ _ => bail!("result.count expected to be a Number, not {:?}", val),
    };
    let page_qt = (count + arg.items_per_page - 1) / arg.items_per_page;

    let items = json.get("result")
        .and_then(|val| val.get("items"))
        .ok_or(anyhow!("could not obtain result.items"))?
    ;
    let items = match items {
        Value::Array(items) => items,
        val @ _ => bail!("result.items expected to be a Array, not {:?}", val),
    };

    let ids = fill_ids(HashSet::new(), items, url)?;

    trace!("items.len(): {}, ids.len: {}", items.len(), ids.len());

    Ok(FetchRet { 
        arg,
        ids,
        page_qt,
    })
}

// ============================================================================

fn fill_ids(ids: Ret, items: &Vec<Value>, url: Url) -> Result<Ret> {
    fill_ids_helper(ids, items, url, &|| "result.items".to_owned())
}

// ============================================================================

fn fill_ids_helper(
    mut ids: Ret, 
    items: &Vec<Value>, 
    url: Url, 
    source_provider: &dyn Fn() -> String,
) -> Result<Ret> {

    for (i, item) in items.iter().enumerate() {
        let item_type = item.get("type")
            .ok_or(anyhow!("could not obtain {}[{}].type", source_provider(), i))?
        ;
        let item_type = match item_type {
            Value::String(item_type) => item_type,
            val @ _ => bail!("{}[{}].type expected to be a Number, not {:?}", source_provider(), i, val),
        };
        match item_type.as_str() {
            "item" | "xlItem" => {
                let id = item.get("value")
                    .and_then(|val| val.get("id"))
                    .ok_or(anyhow!("could not obtain {}[{}].value.id", source_provider(), i))?
                ;
                let id = match id {
                    Value::Number(number) => number.as_u64().ok_or(anyhow!("{}[{}].value.id expected to be u64, not {}", source_provider(), i, number))?,
                    val @ _ => bail!("{}[{}].value.id expected to be a Number, not {:?}", source_provider(), i, val),
                };
                ids.insert(id);
            },
            "vip" => {
                let list = item.get("value")
                    .and_then(|val| val.get("list"))
                    .ok_or(anyhow!("could not obtain {}[{}].value.list", source_provider(), i))?
                ;
                let list = match list {
                    Value::Array(list) => list,
                    val @ _ => bail!("{}[{}].value.list expected to be a Array, not {:?}", source_provider(), i, val),
                };
                ids = fill_ids_helper(ids, list, url.clone(), &|| format!("{}.value.list", source_provider()))?;
            },
            _ => {
                error!("result.items[{}].type: {:?}, at {:?}", i, item_type, url.as_str());
            },
        }
    }
    Ok(ids)
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
// https://m.avito.ru/api/9/items?key=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir&locationId=107620&params[110000]=329202&owner[]=private&sort=default&withImagesOnly=false&lastStamp=1595587140&display=list&page=1&limit=50&priceMax=393073
// https://m.avito.ru/api/9/items?key=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir&page=1&lastStamp=1595584320&display=list&limit=30&url=%2Fmoskva%2Favtomobili%2Fford-ASgBAgICAUTgtg2cmCg%3Fuser%3D1

    // const PARAMS: &str = "locationId=107620&params[110000]=329202&owner[]=private&sort=default&withImagesOnly=false";
    const PARAMS: &str = "locationId=107620&owner[]=private&sort=default&withImagesOnly=false";
    const THREAD_LIMIT_NETWORK: usize = 50;
    const ITEMS_PER_PAGE: usize = 50;

    #[tokio::test]
    async fn it_works() -> Result<()> {
        test_helper::init();

        let diaps_ret_source = r#"{
          "last_stamp": 1595587140,
          "diaps": [
            {
              "price_min": null,
              "price_max": 393073,
              "count": 500,
              "checks": 15
            },
            {
              "price_min": 393074,
              "price_max": 611728,
              "count": 500,
              "checks": 13
            },
            {
              "price_min": 611729,
              "price_max": 896093,
              "count": 491,
              "checks": 13
            },
            {
              "price_min": 896094,
              "price_max": 1248012,
              "count": 471,
              "checks": 13
            },
            {
              "price_min": 1248013,
              "price_max": 1953937,
              "count": 484,
              "checks": 13
            },
            {
              "price_min": 1953938,
              "price_max": null,
              "count": 391,
              "checks": 1
            }
          ],
          "checks_total": 68
        }"#;

        let diaps_ret: diaps::Ret = serde_json::from_str(diaps_ret_source)?;

        // let params = env::get("AVITO_PARAMS", PARAMS.to_owned())?;
        // let thread_limit_network = env::get("AVITO_THREAD_LIMIT_NETWORK", THREAD_LIMIT_NETWORK)?;
        // let items_per_page = env::get("AVITO_ITEMS_PER_PAGE", ITEMS_PER_PAGE)?;
        let params = PARAMS.to_owned();
        let thread_limit_network = THREAD_LIMIT_NETWORK;
        let items_per_page = ITEMS_PER_PAGE;

        let file_path = std::path::Path::new("/cnf/rmq/bikuzin18.toml");
        let settings_rmq = rmq::Settings::new(file_path).map_err(|err| anyhow!("{:?}: {}", file_path, err))?;
        let arg = Arg {
            params: &params,
            diaps_ret: &diaps_ret, 
            thread_limit_network,
            items_per_page,
            client_provider: client::Provider::new(client::Kind::ViaProxy(rmq::get_pool(settings_rmq)?, "ids".to_owned())),
        };
        std::env::set_var("AVITO_AUTH", "af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir");
        let mut auth = auth::Lazy::new(Some(auth::Arg::new()));

        let mut term = Term::init(term::Arg::new().header("Получение списка идентификаторов . . ."))?;
        let start = Instant::now();
        let ids = get(&mut auth, arg, Some(|arg: CallbackArg| -> Result<()> {
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
        println!("{}, Список идентификаторов получен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ids.len());
        Ok(())
    }

}

