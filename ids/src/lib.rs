#![recursion_limit="256"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::time::Instant;
use std::collections::HashSet;

use url::Url;
use http::StatusCode;
use serde_json::Value;

use futures::{
    Future,
    select,
    stream::{
        FuturesUnordered,
        StreamExt,
    },
};

// ============================================================================
// ============================================================================

// pub struct Arg<'a> {
pub struct Arg<'a, F> 
where 
    F: Future<Output=Result<String>>
{
    pub get_auth: fn() -> F, // https://stackoverflow.com/questions/58173711/how-can-i-store-an-async-function-in-a-struct-and-call-it-from-a-struct-instance
    // pub auth: &'a str,
    pub params: &'a str,
    pub diaps_ret: &'a diaps::Ret, 
    pub items_per_page: usize,
    pub thread_limit_network: usize,
    pub retry_count: usize,
}

macro_rules! push_fut {
    ($fut_queue: expr, $client: expr, $auth: expr, $arg: expr, $page: expr, $diap: expr) => {
        let auth = match &$auth {
            Some(auth) => auth.to_owned(),
            None => {
                let auth = ($arg.get_auth)().await?;
                $auth = Some(auth.to_owned());
                auth
            },
        };
        let fetch_arg = FetchArg {
            client: $client,
            auth,
            // auth: $arg.auth.to_owned(),
            diap: $diap, 
            page: $page,
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

// pub async fn get<'a>(
//     arg: Arg<'a>,
pub async fn get<'a, F, Cb>(
    arg: &Arg<'a, F>, 
    mut callback: Option<Cb>,
    // callback: Option<&dyn Fn(CallbackArg) -> Result<()>>,
) -> Result<Ret>
where 
    F: Future<Output=Result<String>>,
    // Cb: FnMut(CallbackArg) -> Result<()>,
    Cb: FnMut(CallbackArg) -> Result<()>,
 {
    let start = Instant::now();

    let mut diap_i = 0;
    let diaps_len = arg.diaps_ret.diaps.len();

    let mut auth: Option<String> = None;
    let mut fut_queue = FuturesUnordered::new();
    while diap_i < arg.thread_limit_network && diap_i < diaps_len {
        let client = reqwest::Client::new();
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
                            ids.insert(id);
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
    client: reqwest::Client,
    auth: String,

    diap: diaps::Diap,
    page: usize,

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
        let text: Result<String>;
        let mut remained = arg.retry_count;
        loop {
            let response = arg.client.get(url.clone()).send().await;
            match response {
                Err(err) => {
                    if remained > 0 {
                        remained -= 1;
                        continue;
                    } else {
                        return Err(Error::new(err))
                    }
                },
                Ok(response) => {
                    match response.status() {
                        StatusCode::OK => {
                            match response.text().await {
                                Ok(t) => {
                                    text = Ok(t);
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
                        },
                        code @ _ => {
                            if remained > 0 {
                                remained -= 1;
                                continue;
                            } else {
                                text = Err(anyhow!("{}: {}", code, response.text().await?));
                                break;
                            }
                        },
                    }
                }
            }
        }
        text
    }.context("ids::fetch")?;
    // let text = {
    //     let text: String;
    //     let mut remained = arg.retry_count;
    //     loop {
    //         let resp = arg.client.get(url.clone()).send().await;
    //         match resp {
    //             Err(err) => {
    //                 if remained > 0 {
    //                     remained -= 1;
    //                     continue;
    //                 } else {
    //                     return Err(Error::new(err))
    //                 }
    //             },
    //             Ok(resp) => {
    //                 match resp.text().await {
    //                     Ok(t) => {
    //                         text = t;
    //                         break;
    //                     },
    //                     Err(err) => {
    //                         if remained > 0 {
    //                             remained -= 1;
    //                             continue;
    //                         } else {
    //                             return Err(Error::new(err))
    //                         }
    //                     },
    //                 }
    //             }
    //         }
    //     }
    //     text
    // };
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
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    const PARAMS: &str = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
    const THREAD_LIMIT_NETWORK: usize = 1;
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

        let params = env::get("AVITO_PARAMS", PARAMS.to_owned())?;
        let thread_limit_network = env::get("AVITO_THREAD_LIMIT_NETWORK", THREAD_LIMIT_NETWORK)?;
        let items_per_page = env::get("AVITO_ITEMS_PER_PAGE", ITEMS_PER_PAGE)?;
        let retry_count = env::get("AVITO_RETRY_COUNT", RETRY_COUNT)?;

        let arg = Arg {
            get_auth,
            params: &params,
            diaps_ret: &diaps_ret, 
            thread_limit_network,
            items_per_page,
            retry_count,
        };

        helper(&arg).await?;
        // let cmd = ansi_escapes::EraseLines(2);
        // let (_, mut row_prev) = crossterm::cursor::position()?;
        // println!("ids::get");
        // let (mut col_last, mut row_last) = crossterm::cursor::position();
        // let ids = get(&arg, Some(&|arg: CallbackArg| -> Result<()> {
        //     let (col_new, row_new) = crossterm::cursor::position()?;
        //     if row_new == row_last && col_new == col_last {
        //         for _ in 0..=row_new - row_prev + 1 {
        //             println!(ansi_escapes::EraseLines(2));
        //         }
        //     }
        //     row_prev = crossterm::cursor::position()?.1;
        //     println!("ids::get: time: {}/{}-{}, per: {}, qt: {}/{}-{}, ids_len: {}", 
        //         arrange_millis::get(arg.elapsed_millis), 
        //         arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
        //         arrange_millis::get(arg.remained_millis), 
        //         arrange_millis::get(arg.per_millis), 
        //         arg.elapsed_qt,
        //         arg.elapsed_qt + arg.remained_qt,
        //         arg.remained_qt,
        //         arg.ids_len,
        //     );
        //     (col_last, row_last) = crossterm::cursor::position()?;
        //     Ok(())
        // })).await?;
        // println!("{}", cmd);
        // println!("ids_len: {}", ids.len());
        
        Ok(())
    }

use std::io::{stdout
    // , Write
};

use crossterm::{
    ExecutableCommand, 
    terminal, 
    cursor,
};

    async fn helper<'a, F>(arg: &Arg<'a, F>) -> Result<()> 
    where F: Future<Output=Result<String>>,
    {

        let mut row_prev = crossterm::cursor::position()?.1;
        let rows = crossterm::terminal::size()?.1;
        if row_prev == rows - 1 {
            row_prev -= 2;
            stdout().execute(terminal::ScrollUp(2))?;
            stdout().execute(cursor::MoveTo(0, row_prev))?;
        }
        println!("ids::get");
        let mut row_last = crossterm::cursor::position()?.1;

        let ids = get(&arg, Some(|arg: CallbackArg| -> Result<()> {

            let (col_new, row_new) = crossterm::cursor::position()?;
            if row_new == row_last && col_new == 0 {
                stdout().execute(cursor::MoveTo(0, row_prev))?;
            }

            row_prev = crossterm::cursor::position()?.1;
            let rows = crossterm::terminal::size()?.1;
            if row_prev == rows - 1 {
                row_prev -= 2;
                stdout().execute(terminal::ScrollUp(2))?;
                stdout().execute(cursor::MoveTo(0, row_prev))?;
            }

            println!("ids::get: time: {}/{}-{}, per: {}, qt: {}/{}-{}, ids_len: {}", 
                arrange_millis::get(arg.elapsed_millis), 
                arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                arrange_millis::get(arg.remained_millis), 
                arrange_millis::get(arg.per_millis), 
                arg.elapsed_qt,
                arg.elapsed_qt + arg.remained_qt,
                arg.remained_qt,
                arg.ids_len,
            );
            row_last = crossterm::cursor::position()?.1;

            Ok(())
        })).await?;
        println!("ids_len: {}", ids.len());
        Ok(())
    }

    async fn get_auth() -> Result<String> {
        println!("Получение токена авторизации . . .");
        let start = Instant::now();
        let auth = auth::get().await?;
        println!("Токен авторизации получен, {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()));
        Ok(auth)
    }
}

