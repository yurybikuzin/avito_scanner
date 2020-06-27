#![recursion_limit="256"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::time::Instant;
use std::collections::HashSet;
use url::Url;

use futures::{
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};
use serde_json::Value;

pub struct Arg<'a> {
    pub auth: &'a str,
    pub params: &'a str,
    pub diaps_ret: &'a diaps::Ret, 
    pub thread_limit: usize,
    pub items_per_page: usize,
    pub retry_count: usize,
}

macro_rules! push_fut {
    ($fut_queue: expr, $client: expr, $arg: expr, $page: expr, $diap: expr) => {
        let fetch_arg = FetchArg {
            client: $client,
            diap: $diap, 
            page: $page,
            auth: $arg.auth.to_owned(),
            params: $arg.params.to_owned(),
            last_stamp: $arg.diaps_ret.last_stamp,
            items_per_page: $arg.items_per_page,
            retry_count: $arg.retry_count,
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
pub async fn get<'a>(
    arg: Arg<'a>,
    callback: &dyn Fn(CallbackArg),
) -> Result<Ret> {
    let now = Instant::now();

    let mut diaps_i = 0;
    let diaps_len = arg.diaps_ret.diaps.len();

    let mut fut_queue = FuturesUnordered::new();
    while diaps_i < arg.thread_limit && diaps_i < diaps_len {
        let client = reqwest::Client::new();
        let diap = arg.diaps_ret.diaps[diaps_i].clone();
        push_fut!(fut_queue, client, arg, 1, diap);
        diaps_i += 1;
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
                            ids.insert(id);
                        }
                        elapsed_qt += 1;
                        if remained_qt > 0 {
                            remained_qt -= 1;
                        }
                        let elapsed_millis = Instant::now().duration_since(now).as_millis(); 
                        let per_millis = elapsed_millis / elapsed_qt as u128;
                        let remained_millis = per_millis * remained_qt as u128;
                        callback(CallbackArg {
                            elapsed_qt,
                            remained_qt,
                            elapsed_millis, 
                            remained_millis, 
                            per_millis,
                            ids_len: ids.len()
                        });
                        if ret.arg.page < ret.page_qt {
                            push_fut!(fut_queue, ret.arg.client, arg, ret.arg.page + 1, ret.arg.diap);
                        } else if diaps_i < diaps_len {
                            push_fut!(fut_queue, ret.arg.client, arg, 1, arg.diaps_ret.diaps[diaps_i].clone());
                            diaps_i += 1;
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
        Instant::now().duration_since(now).as_millis(), 
        ids.len(),
    );
    
    Ok(ids)
}

struct FetchArg {
    client: reqwest::Client,

    diap: diaps::Diap,
    page: usize,

    auth: String,
    params: String,
    last_stamp: u64,
    items_per_page: usize,
    retry_count: usize,
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
    let text = {
        let text: String;
        let mut remained = arg.retry_count;
        loop {
            let resp = arg.client.get(url.clone()).send().await;
            match resp {
                Err(err) => {
                    if remained > 0 {
                        remained -= 1;
                        continue;
                    } else {
                        return Err(Error::new(err))
                    }
                },
                Ok(resp) => {
                    match resp.text().await {
                        Ok(t) => {
                            text = t;
                            break;
                        },
                        Err(err) => {
                            if remained > 0 {
                                remained -= 1;
                                continue;
                            } else {
                                return Err(Error::new(err))
                            }
                        },
                    }
                }
            }
        }
        text
    };
    trace!("{} ms , ids::fetch({:?})", Instant::now().duration_since(now).as_millis(), url.as_str());

    let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("failed to parse json from: {}", text))?;

    let count = json.get("result")
        .and_then(|val| val.get("count"))
        .ok_or(anyhow!("could not obtain result.count"))?
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

fn fill_ids(ids: Ret, items: &Vec<Value>, url: Url) -> Result<Ret> {
    fill_ids_helper(ids, items, url, &|| "result.items".to_owned())
}

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

    const PARAMS: &str = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
    const THREAD_LIMIT: usize = 1;
    const ITEMS_PER_PAGE: usize = 50;
    const RETRY_COUNT: usize = 3;

    #[tokio::test]
    async fn it_works() -> Result<()> {
        init();

        let diaps_ret_source = r#"{
          "last_stamp": 1593162840,
          "diaps": [
            {
              "price_min": null,
              "price_max": 234376,
              "count": 4741,
              "checks": 7
            },
            {
              "price_min": 234377,
              "price_max": 379514,
              "count": 4839,
              "checks": 9
            },
            {
              "price_min": 379515,
              "price_max": 520615,
              "count": 4720,
              "checks": 9
            },
            {
              "price_min": 520616,
              "price_max": 739205,
              "count": 4711,
              "checks": 8
            },
            {
              "price_min": 739206,
              "price_max": 1200685,
              "count": 4822,
              "checks": 12
            },
            {
              "price_min": 1200686,
              "price_max": 2200686,
              "count": 3378,
              "checks": 2
            },
            {
              "price_min": 2200687,
              "price_max": null,
              "count": 1735,
              "checks": 1
            }
          ],
          "checks_total": 48
        }"#;

        let diaps_ret: diaps::Ret = serde_json::from_str(diaps_ret_source)?;

        let auth = auth::get().await?;

        let params = env::get("AVITO_PARAMS", PARAMS.to_owned())?;
        let thread_limit = env::get("AVITO_THREAD_LIMIT", THREAD_LIMIT)?;
        let items_per_page = env::get("AVITO_ITEMS_PER_PAGE", ITEMS_PER_PAGE)?;
        let retry_count = env::get("AVITO_RETRY_COUNT", RETRY_COUNT)?;

        let arg = Arg {
            auth: &auth,
            params: &params,
            diaps_ret: &diaps_ret, 
            thread_limit,
            items_per_page,
            retry_count,
        };

        println!("ids::get");
        let ids = get(arg, &|arg: CallbackArg| {
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
        println!("{}", ansi_escapes::EraseLines(2));
        println!("ids_len: {}", ids.len());
        
        Ok(())
    }

}
