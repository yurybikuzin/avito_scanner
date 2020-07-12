
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::fmt;
use std::time::Instant;
use url::Url;
use serde_json::Value;

#[macro_use]
extern crate serde;

// ============================================================================
// ============================================================================

/// Аргумент для fn get
#[derive(Clone)]
pub struct Arg<'a> {
    /// params - непустая строка параметров, например: "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private"
    pub params: &'a str, 

    /// count_limit - Максимальное количество объявлений в диапазоне, например 4900
    pub count_limit: u64, 

    /// price_precision - Минимальный размер шага изменения верхней границы диапазона в процессе обнаружения диапазона, используется в условии останова, например 20000
    pub price_precision: isize, 

    /// price_max_inc - Величина, используемая при формировании первичного значения верхней границы диапазона на основе значения нижней, например 1000000
    pub price_max_inc: isize,

    pub client_provider: client::Provider,
}

// ============================================================================

impl fmt::Display for Arg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}:{}", self.params, self.count_limit, self.price_precision, self.price_max_inc)
    }
}

// ============================================================================
// ============================================================================

/// Значение, возвращаемое fn get
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ret {
    /// lastStamp
    pub last_stamp: u64,

    /// список диапазонов
    pub diaps: Vec<Diap>,

    /// количество запросов к серверу, сделанных для форирования диапазонов
    pub checks_total: usize,
}

// ============================================================================
// ============================================================================

/// Диапазон цены, содержащий близкое к get::PARAMS::count_limit число объявлений, но не больше
#[derive(Debug)]
#[derive(Serialize, Deserialize, Clone)]
pub struct Diap {
    /// Нижняя граница диапазона
    pub price_min: Option<isize>,
    /// Верхняя граница диапазона
    pub price_max: Option<isize>,
    /// Количество объявлений в диапазоне
    pub count: u64,
    /// Количество запросов к серверу при формировании диапазона
    pub checks: usize,
}

// ============================================================================

pub struct CallbackArg {
    pub count_total: u64,
    pub checks_total: usize,
    pub elapsed_millis: u128,
    pub per_millis: u128,
    pub diaps_detected: usize,
    pub price_min: Option<isize>,
    pub price_max: Option<isize>,
    pub price_max_delta: Option<isize>,
    pub count: Option<u64>,
}

macro_rules! callback {
    ($callback: expr, $start: expr, $count_total: expr, $checks_total: expr, $diaps: expr, $count: expr, $price_min: expr, $price_max: expr, $price_max_delta: expr) => {
        let elapsed_millis = Instant::now().duration_since($start).as_millis();
        let per_millis = elapsed_millis / $checks_total as u128;
        $callback(CallbackArg {
            count_total: $count_total.unwrap(),
            checks_total: $checks_total,
            elapsed_millis,
            per_millis,
            diaps_detected: $diaps.len(),
            count: $count,
            price_min: $price_min,
            price_max: $price_max,
            price_max_delta: $price_max_delta,
        })?;
    };
}

// ============================================================================

/// Возвращает список диапазон цен, каждый из которых содержит близкое к arg.count_limit объявлений, но
/// не больше
///
/// Формирование диапазона прекращается, когда на очередной итерации количество объявлений в
/// диапазоне не превысило arg.count_limit и при этом шаг изменения верхней гранциы диапазона либо
/// неопределен (первая итерация), либо не превышает значения arg.price_precision
///
/// При этом, 
///     первоначальное значение нижней границы очередного диапазона устанавливается равным 
///     значению верхней границы предыдущего диапазон плюс один
///     первоаначальное значение верхней границы очередоного диапазона устанавливается равным
///     значению нижней границы плюс arg.price_max_inc
///
/// # Params:
///
/// - auth - непустой ключ авторизации (получаем у сервиса auth)
/// - arg - другие аргументы, согласно Arg
///
///
/// # Return:
///
/// Возвращает Ret
///
/// # Errors
///
/// Возвращает ошибки, если возникли проблемы при соединении с сервером, преобразовании ответа в
/// json, извлечении значений result.count и result.lastStamp
pub async fn get<'a, Cb>(
    auth: &mut auth::Lazy,
    arg: Arg<'a>,
    mut callback: Option<Cb>,
) -> Result<Ret> 
where 
    Cb: FnMut(CallbackArg) -> Result<()>,
{
    let Arg {params, count_limit, price_precision, price_max_inc, client_provider} = arg;

    let start = Instant::now();

    let mut price_min: Option<isize> = None;
    let mut price_max: Option<isize> = None;
    let mut price_max_delta: Option<isize> = None;
    let mut count: Option<u64> = None;
    let mut diaps: Vec<Diap> = vec![];
    let mut last_stamp: Option<u64> = None;
    let mut checks_total: usize = 0;
    let client = client_provider.build().await?;
    let mut count_total: Option<u64> = None;
    loop {
        let mut checks: usize = 0;
        while need_continue(count, price_max_delta, count_limit, price_precision) {
            let url = format!(
                "https://avito.ru/api/9/items?key={}&{}&display=list&page=1&limit=1{}{}{}",
                auth.key().await?,
                params,         
                match price_min { None => "".to_owned(), Some(price_min) => format!("&priceMin={}", price_min) },
                match price_max { None => "".to_owned(), Some(price_max) => format!("&priceMax={}", price_max) },
                match last_stamp { None => "".to_owned(), Some(last_stamp) => format!("&lastStamp={}", last_stamp) },
            );
            let url = Url::parse(&url)?;
            let (text, status) = client.get_text_status(url.clone()).await?;
            if status != http::StatusCode::OK {
                bail!("url: {}, status: {}", url, status);
            }
            checks += 1;
            checks_total += 1;
            let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("failed to parse json from: {}", text))?;
            let last_stamp_src = json.get("result")
                .and_then(|val| val.get("lastStamp"))
                .ok_or(anyhow!("could not obtain result.lastStamp"))?
            ;
            let last_stamp_src = match last_stamp_src {
                Value::Number(number) => number.as_u64().ok_or(anyhow!("result.lastStamp expected to be u64, not {}", number))?,
                val @ _ => bail!("result.lastStamp expected to be a Number, not {:?}", val),
            };
            last_stamp = Some(last_stamp_src);

            let count_src = json.get("result")
                .and_then(|val| val.get("count"))
                .ok_or(anyhow!("could not obtain result.count"))?
            ;
            let count_src = match count_src {
                Value::Number(number) => number.as_u64().ok_or(anyhow!("result.count expected to be u64, not {}", number))?,
                val @ _ => bail!("result.count expected to be a Number, not {:?}", val),
            };
            count = Some(count_src);
            if count_total.is_none() {
                count_total = count;
            }

            callback = if let Some(mut callback) = callback {
                callback!(callback, start, count_total, checks_total, diaps, count, price_min, price_max, price_max_delta);

                Some(callback)
            } else {
                None
            };

            trace!("count: {:?}", count);
            if let Some(count) = count {

                let (price_max_new, price_max_delta_new) = 
                    if count > count_limit {
                        if let Some(price_max) = price_max {
                            let price_max_new = if let Some(price_max_delta) = price_max_delta {
                                price_max - if price_max_delta > 0 {
                                    div_round_closest(price_max_delta, 2)
                                } else {
                                    - div_round_closest(price_max_delta, 2)
                                }
                            } else {
                                let price_diap = price_max - price_min.unwrap_or(0);
                                div_round_closest(price_diap, 2)
                            };
                            let price_max_delta_new = price_max_new - price_max;
                            (Some(price_max_new), Some(price_max_delta_new))
                        } else {
                            let price_max_new = price_min.unwrap_or(0) + price_max_inc;
                            (Some(price_max_new), price_max_delta)
                        }
                    } else if count < count_limit {
                        if let Some(price_max) = price_max {
                            if let Some(price_max_delta) = price_max_delta {
                                let price_max_new = price_max + if price_max_delta > 0 {
                                    div_round_closest(price_max_delta, 2)
                                } else {
                                    - div_round_closest(price_max_delta, 2)
                                };
                                (Some(price_max_new), Some(price_max_new - price_max))
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break
                    }
                ;
                price_max = price_max_new;
                price_max_delta = price_max_delta_new;
                trace!("price_min: {:?}, price_max: {:?}, price_max_delta: {:?}", price_min, price_max, price_max_delta);
            } else {
                unreachable!();
            }
        }
        let diap = Diap {price_min, price_max, count: count.unwrap(), checks};
        trace!(">>> diap: {:?}", diap);
        diaps.push(diap);
        price_min = Some(
            match price_max {
                None => break,
                Some(price_max) => price_max + 1,
            }
        );
        price_max = None;
        price_max_delta = None;
        count = None;
    }
    if let Some(mut callback) = callback {
        callback!(callback, start, count_total, checks_total, diaps, count, price_min, price_max, price_max_delta);
    }

    Ok(Ret {
        last_stamp: last_stamp.unwrap(),
        diaps,
        checks_total,
    })
}

// ============================================================================
// ============================================================================

/// Целочисленное деление с округлением
/// https://stackoverflow.com/questions/2422712/rounding-integer-division-instead-of-truncating/2422723#2422723
fn div_round_closest(dividend: isize, divisor: isize) -> isize {
    (dividend + (divisor / 2)) / divisor
}

// ============================================================================

/// Условие продолжения цикла формирования диапазона
fn need_continue(
    count: Option<u64>, 
    price_max_delta: Option<isize>, 
    count_limit: u64, 
    price_precision: isize,
) -> bool {
    match count {
        None => true,
        Some(count) => {
            count > count_limit ||
            count < count_limit && match price_max_delta {
                None => false,
                Some(price_max_delta) => price_max_delta.abs() > price_precision,
            }
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

    const COUNT_LIMIT: u64 = 4900;
    const PRICE_PRECISION: isize = 20000;
    const PRICE_MAX_INC: isize = 1000000;

    use term::Term;

    // docker exec -it -e RUST_LOG=diaps=trace -e AVITO_AUTH=af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir avito-proj cargo test -p diaps -- --nocapture
    #[tokio::test]
    async fn test_diaps() -> Result<()> {
        test_helper::init();

        let client_provider = client::Provider::new(client::Kind::Reqwest(1));
        helper(client_provider).await?;

        // let pool = rmq::get_pool();
        // let client_provider = client::Provider::new(client::Kind::ViaProxy(pool));
        // helper(client_provider).await?;

        Ok(())
    }

    async fn helper(client_provider: client::Provider) -> Result<()> {

        let count_limit: u64 = env::get("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
        let price_precision: isize = env::get("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
        let price_max_inc: isize = env::get("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;


        let params = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
        let arg = Arg { 
            params, 
            count_limit, 
            price_precision, 
            price_max_inc,
            client_provider,
        };


        let mut term = Term::init(term::Arg::new().header("Определение диапазонов цен . . ."))?;
        let start = Instant::now();
        let mut auth = auth::Lazy::new(Some(auth::Arg::new()));
        let ret = get(&mut auth, arg, Some(|arg: CallbackArg| -> Result<()> {
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
        })).await?;
        println!("{}, Определены диапазоны цен: {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()), ret.diaps.len());

        info!("ret:{}", serde_json::to_string_pretty(&ret).unwrap());

        Ok(())
    }
}

