
use log::{info};
use anyhow::{bail, Result, anyhow};
use serde_json::Value;
use url::Url;
use std::env;
use std::fmt;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

// ============================================================================
// ============================================================================

/// Аргумент для fn get
pub struct GetArg<'a> {
    /// params - непустая строка параметров, например: "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private"
    pub params: &'a str, 

    /// count_limit - Максимальное количество объявлений в диапазоне, например 4900
    pub count_limit: u64, 

    /// price_precision - Минимальный размер шага изменения верхней границы диапазона в процессе обнаружения диапазона, используется в условии останова, например 20000
    pub price_precision: isize, 

    /// price_max_inc - Величина, используемая при формировании первичного значения верхней границы диапазона на основе значения нижней, например 1000000
    pub price_max_inc: isize,
}

// ============================================================================

impl fmt::Display for GetArg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}:{}", self.params, self.count_limit, self.price_precision, self.price_max_inc)
    }
}

// ============================================================================
// ============================================================================

/// Значение, возвращаемое fn get
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GetRet {
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
/// - key - непустой ключ авторизации (получаем у сервиса auth)
/// - arg - другие аргументы, согласно GetArg
///
///
/// # Return:
///
/// Возвращает GetRet
///
/// # Errors
///
/// Возвращает ошибки, если возникли проблемы при соединении с сервером, преобразовании ответа в
/// json, извлечении значений result.count и result.lastStamp
pub async fn get<'a>(key: &'a str, arg: GetArg<'a>) -> Result<GetRet> {
    let GetArg {params, count_limit, price_precision, price_max_inc} = arg;

    let mut price_min: Option<isize> = None;
    let mut price_max: Option<isize> = None;
    let mut price_max_delta: Option<isize> = None;
    let mut count: Option<u64> = None;
    let mut diaps: Vec<Diap> = vec![];
    let mut last_stamp: Option<u64> = None;
    let mut checks_total: usize = 0;
    let client = reqwest::Client::new();
    loop {
        let mut checks: usize = 0;
        while need_continue(count, price_max_delta, count_limit, price_precision) {
            let url = Url::parse(&format!(
                "https://avito.ru/api/9/items?key={}&{}&display=list&page=1&limit=1{}{}{}",
                key,
                params,
                match price_min { None => "".to_owned(), Some(price_min) => format!("&priceMin={}", price_min) },
                match price_max { None => "".to_owned(), Some(price_max) => format!("&priceMax={}", price_max) },
                match last_stamp { None => "".to_owned(), Some(last_stamp) => format!("&lastStamp={}", last_stamp) },
            ))?;
            let text = client.get(url).send().await?.text().await?;
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

            info!("count: {:?}", count);
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
                info!("price_min: {:?}, price_max: {:?}, price_max_delta: {:?}", price_min, price_max, price_max_delta);
            } else {
                unreachable!();
            }
        }
        let diap = Diap {price_min, price_max, count: count.unwrap(), checks};
        info!(">>> diap: {:?}", diap);
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
    info!("checks_total: {:?}", checks_total);
    Ok(GetRet {
        last_stamp: last_stamp.unwrap(),
        diaps,
        checks_total,
    })
}

// ============================================================================
// ============================================================================

/// Возвращает значение переменной среды env_var_name
/// Если переменная среды не определена, возвращает значение по умолчанию default
///
/// #Errors
///
/// Если значение переменноё среды определено, но не удалось преобразовать в тип T, возвращает
/// ошибку
pub fn param<T: std::str::FromStr>(env_var_name: &str, default: T) -> Result<T> {
    env::var(env_var_name)
    .map_or_else(
        |_err| Ok(default), 
        |val| val.parse::<T>().map_err(
            |_err| anyhow!("failed to parse {}='{}'", env_var_name, val)
        ),
    )
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
    use super::*;
    use env_logger;
    use http::StatusCode;
    use std::time::Instant;

    const COUNT_LIMIT: u64 = 4900;
    const PRICE_PRECISION: isize = 20000;
    const PRICE_MAX_INC: isize = 1000000;

    #[tokio::test]
    async fn it_works() -> Result<()> {
        env_logger::init();

        let count_limit: u64 = param("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
        let price_precision: isize = param("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
        let price_max_inc: isize = param("AVITO_PRICE_MAX_INC", PRICE_MAX_INC)?;

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

        let now = Instant::now();
        let params = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";

        let GetRet {last_stamp, diaps, checks_total} = get( &key, GetArg { 
            params, 
            count_limit, 
            price_precision, 
            price_max_inc,
        }).await?;

        info!("({} ms) last_stamp: {}, diaps: {:?}, len: {:?}, total_checks: {:?}", 
            Instant::now().duration_since(now).as_millis(),
            last_stamp, 
            diaps, 
            diaps.len(), 
            checks_total,
        );

        Ok(())
    }
}

