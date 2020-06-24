use log::{info};
use anyhow::{bail, 
    // ensure, Context, 
    Result, anyhow};
use serde_json::Value;
use url::Url;

#[derive(Debug)]
pub struct Diap {
    pub price_max: Option<isize>,
    pub price_min: Option<isize>,
    pub count: u64,
    checks: usize,
}

const PRICE_PRECISION: isize = 20000;
const PRICE_MAX_INC: isize = 1000000;
const COUNT_LIMIT: u64 = 4900;

// https://stackoverflow.com/questions/2422712/rounding-integer-division-instead-of-truncating/2422723#2422723
fn div_round_closest(dividend: isize, divisor: isize) -> isize {
    (dividend + (divisor / 2)) / divisor
}

fn need_continue(count: Option<u64>, price_max_delta: Option<isize>) -> bool {
    match count {
        None => true,
        Some(count) => {
            count > COUNT_LIMIT ||
            count < COUNT_LIMIT && match price_max_delta {
                None => false,
                Some(price_max_delta) => price_max_delta.abs() > PRICE_PRECISION,
            }
        },
    }
}

pub async fn get(key: &str) -> Result<Vec<Diap>> {
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
        while need_continue(count, price_max_delta) {
            let url = Url::parse(&format!(
                "https://m.avito.ru/api/9/items?key={}&categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private&page=1&display=list&page=1&limit=1{}{}{}",
                key,
                match price_min { None => "".to_owned(), Some(price_min) => format!("&priceMin={}", price_min) },
                match price_max { None => "".to_owned(), Some(price_max) => format!("&priceMax={}", price_max) },
                match last_stamp { None => "".to_owned(), Some(last_stamp) => format!("&lastStamp={}", last_stamp) },
            ))?;
            let text = client.get(url).send().await?.text().await?;
            checks += 1;
            checks_total += 1;
            let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("failed to parse json from: {}", text))?;
            last_stamp = Some(match json.get("result")
                .and_then(|val| val.get("lastStamp"))
                .ok_or(anyhow!("could not obtain result.lastStamp"))? {
                    Value::Number(number) => number.as_u64().ok_or(anyhow!("result.lastStamp expected to be u64, not {}", number))?,
                    val @ _ => bail!("result.lastStamp expected to be a Number, not {:?}", val),
                });
            count = Some(match json.get("result")
                .and_then(|val| val.get("count"))
                .ok_or(anyhow!("could not obtain result.count"))? {
                    Value::Number(number) => number.as_u64().ok_or(anyhow!("result.count expected to be u64, not {}", number))?,
                    val @ _ => bail!("result.count expected to be a Number, not {:?}", val),
                });

            info!("count: {:?}", count);
            if let Some(count) = count {

                let (price_max_new, price_max_delta_new) = 
                    if count > COUNT_LIMIT {
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
                            let price_max_new = price_min.unwrap_or(0) + PRICE_MAX_INC;
                            (Some(price_max_new), price_max_delta)
                        }
                    } else if count < COUNT_LIMIT {
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
    Ok(diaps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;
    use http::StatusCode;

    #[tokio::test]
    async fn it_works() -> Result<()> {
        env_logger::init();
        let response = reqwest::get("http://auth:3000").await?;
        let key = match response.status() {
            StatusCode::OK => {
                response.text().await?
            },
            StatusCode::NOT_FOUND => {
                bail!("NOT_FOUND: {}", response.text().await?);
            },
            StatusCode::INTERNAL_SERVER_ERROR => {
                bail!("INTERNAL_SERVER_ERROR: {}", response.text().await?);
            },
            _ => {
                unreachable!();
            },
        };
        // info!("key!: {}", key);
        let diaps = get(&key).await?;
        info!("diaps!: {:?}", diaps);

        Ok(())
    }
}
