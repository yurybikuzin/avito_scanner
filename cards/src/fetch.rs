
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;
use http::StatusCode;
use serde_json::{Value, Map, Number};

use regex::Regex;

use std::time::Instant;

pub struct Arg {
    pub client: reqwest::Client,
    pub auth: String,
    pub id: u64,
    pub retry_count: usize,
}

pub struct Ret {
    pub client: reqwest::Client,
    pub id: u64,
    pub json: Option<Value>,
}

pub async fn run(arg: Arg) -> Result<Ret> {
    let url = &format!("https://avito.ru/api/14/items/{}?key={}", 
        arg.id,
        arg.auth, 
    );
    let url = Url::parse(&url)?;

    let now = Instant::now();
    let text = {
        let text: Result<Option<String>>;
        let mut remained = arg.retry_count;
        loop {
            let response = arg.client.get(url.clone()).send().await;
            match response {
                Err(err) => {
                    if remained > 0 {
                        remained -= 1;
                        continue;
                    } else {
                        error!("{}: {:?}", url, err);
                        text = Err(Error::new(err));
                        break;
                    }
                },
                Ok(response) => {
                    match response.status() {
                        StatusCode::OK => {
                            match response.text().await {
                                Ok(t) => {
                                    text = Ok(Some(t));
                                    break;
                                },
                                Err(err) => {
                                    if remained > 0 {
                                        remained -= 1;
                                        continue;
                                    } else {
                                        error!("{}: {:?}", url, err);
                                        text = Err(Error::new(err));
                                        break;
                                    }
                                },
                            }
                        },
                        code @ StatusCode::NOT_FOUND => {
                            let msg = response.text().await?;
                            warn!("{} :: {}: {}", url, code, msg);
                            text = Ok(None);
                            break;
                        },
                        code @ _ => {
                            if remained > 0 {
                                remained -= 1;
                                continue;
                            } else {
                                let msg = response.text().await?;
                                error!("{} :: {}: {}", url, code, msg);
                                text = Err(anyhow!("{} :: {}: {}", url, code, msg));
                                break;
                            }
                        },
                    }
                }
            }
        }
        text
    }.context("cards::fetch")?;
    trace!("{}, cards::fetch({:?})", arrange_millis::get(Instant::now().duration_since(now).as_millis()), url);
    let json = match text {
        None => None,
        Some(text) => {

            let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("{} :: failed to parse json from: {}", url, text))?;
            let json = match json {
                Value::Object(map) => map,
                val @ _ => bail!("{} :: response json expected to be an Object, not {:?}", url, val),
            };

            let mut map = Map::new();
            for (key, val) in json.iter() {
                match key.as_str() {
                    "address" | "adjustParams" | "categoryId" | "coords" | "districtId" | "geoReferences" | "locationId" | "metroId" | "metroType" | "time" | "userType" | "titleGenerated" | "title" | "stats" | "sharing" | "seller" | "needToCheckCreditInfo" | "needToCheckSimilarItems" | "images" | "video" | "icebreakers" | "contacts" | "autotekaTeaser" | "autoCatalogAction" | "anonymousNumber" | "vehicleType" | "parameters" | "shouldLogAction" | "price" | "similarAction" | "shopId" | "shopType" => {
                        // skip
                    },
                    "status" | "closingReason" => {
                        map.insert(key.to_owned(), val.clone());
                    },
                    "id" => {
                        map.insert("avitoId".to_owned(), val.clone());
                    },
                    "autoCatalogUrl" | "description" | "features" => {
                        map.insert(key.to_owned(), val.clone());
                    },
                    "priceBadge" => {
                        let sub_key = "marketPrice";
                        match json.get(key).and_then(|val| val.get(sub_key)) {
                            None => {
                                error!("{} :: {}.{}: could not obtain", url, key, sub_key);
                            },
                            Some(val) => {
                                let val = match val {
                                    Value::String(s) => {
                                        lazy_static! {
                                            static ref RE: Regex = Regex::new(r"[^\d]").unwrap();
                                        }
                                        Value::Number(
                                            Number::from_f64(
                                                RE.replace_all(s, "").parse::<u64>()
                                                    .map_err(|err| Error::new(err))
                                                    .context(format!("{} :: {}.{} expected to be a String parsable to Number, not {:?}", url, key, sub_key, s))? as f64
                                            ).unwrap()
                                        )
                                    },
                                    Value::Number(n) => Value::Number(n.clone()),
                                    val @ _ => bail!("{} :: {}.{} expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                };
                                map.insert(sub_key.to_owned(), val.clone());
                            },
                        }
                    },
                    "seo" => {
                        let sub_key = "canonicalUrl";
                        match json.get(key).and_then(|val| val.get(sub_key)) {
                            None => {
                                error!("{} :: {}.{}: could not obtain", url, key, sub_key);
                            },
                            Some(val) => {
                                map.insert(sub_key.to_owned(), val.clone());
                            },
                        }
                    },
                    "firebaseParams" => {
                        let firebase_params = match val {
                            Value::Object(map) => map,
                            val @ _ => bail!("response json '{:?}' expected to be an Object, not {:?}", key, val),
                        };
                        for (sub_key, val) in firebase_params.iter() {
                            match sub_key.as_str() {
                                "body_type" => {
                                    map.insert("bodyType".to_owned(), val.clone());
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("response json '{}.{}' expected to be an String, not {:?}", key, sub_key, val),
                                    };
                                    let val = match val.as_str() {
                                        "Внедорожник" => "внедорожник",
                                        "Седан" => "седан",
                                        "Купе" => "купе",
                                        "Хетчбэк" => "хэтчбек",
                                        "Лифтбек" => "лифтбек",
                                        "Универсал" => "универсал",
                                        "Минивэн" => "минивэн",
                                        "Кабриолет" => "кабриолет",
                                        "Фургон" => "фургон",
                                        "Пикап" => "пикап",
                                        "Микроавтобус" => "микроавтобус",
                                        s @ _ => {
                                            error!("{} :: {}.{}: {:?}", url, key, sub_key, s);
                                            s
                                        }
                                    };
                                    map.insert("vehicleTransmission".to_owned(), Value::String(val.to_owned()));
                                },
                                "engine_type" => {
                                    map.insert("fuelType".to_owned(), val.clone());
                                },
                                "transmission" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("response json '{}.{}' expected to be an String, not {:?}", key, sub_key, val),
                                    };
                                    let val = match val.as_str() {
                                        "Механика" => "механическая",
                                        "Автомат" => "автоматическая",
                                        "Вариатор" => "вариатор",
                                        "Робот" => "робот",
                                        s @ _ => {
                                            error!("{} :: {}.{}: {}", url, key, sub_key, s);
                                            s
                                        }
                                    };
                                    map.insert("vehicleTransmission".to_owned(), Value::String(val.to_owned()));
                                },
                                "drive" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: {}.{} expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    let val = match val.as_str() {
                                        "Передний" => "передний",
                                        "Задний" => "задний",
                                        "Полный" => "полный",
                                        s @ _ => {
                                            error!("{} :: {}.{}: {}", url, key, sub_key, s);
                                            s
                                        }
                                    };
                                    map.insert("Привод".to_owned(), Value::String(val.to_owned()));
                                },
                                "elektrosteklopodemniki" => {
                                    map.insert("Электростеклоподъемники".to_owned(), val.clone());
                                },
                                "usilitel_rulya" => {
                                    map.insert("Усилитель руля".to_owned(), val.clone());
                                },
                                "diski" => {
                                    map.insert("Диски".to_owned(), val.clone());
                                },
                                "salon" => {
                                    map.insert("Салон".to_owned(), val.clone());
                                },
                                "audiosistema" => {
                                    map.insert("Аудиосистема".to_owned(), val.clone());
                                },
                                "wheel" => {
                                    map.insert("Руль".to_owned(), val.clone());
                                },
                                "condition" => {
                                    map.insert("Состояние".to_owned(), val.clone());
                                },
                                "year" => {
                                    map.insert("productionDate".to_owned(), val.clone());
                                },
                                "model" => {
                                    map.insert("name".to_owned(), val.clone());
                                },
                                "vladeltsev_po_pts" => {
                                    map.insert("Владельцы".to_owned(), val.clone());
                                },
                                "upravlenie_klimatom" => {
                                    map.insert("Климат-контроль".to_owned(), val.clone());
                                },
                                "fary" => {
                                    map.insert("Фары".to_owned(), val.clone());
                                },
                                "itemPrice" | "kolichestvo_dverey" => {
                                    let val = match val {
                                        Value::String(s) => Value::Number(
                                            Number::from_f64(
                                                s.parse::<u64>()
                                                    .map_err(|err| Error::new(err))
                                                    .context(format!("response json '{}.{}' expected to be a String parsable to Number, not {:?}", key, sub_key, s))? as f64
                                            ).unwrap()
                                        ),
                                        Value::Number(n) => Value::Number(n.clone()),
                                        val @ _ => bail!("response json '{}.{}' expected to be a Number or a String, not {:?}", key, sub_key, val),
                                    };
                                    let key_new = match sub_key.as_str() {
                                        "kolichestvo_dverey" => "numberOfDoors",
                                        "itemPrice" => "itemPrice",
                                        _ => unreachable!(),

                                    };
                                    map.insert(key_new.to_owned(), val);
                                },
                                "brand" | "color" | "mileage" | "capacity" | "engine" | "type_of_trade" 
         => {

                                            map.insert(key.to_owned(), val.clone());
                                },
                                "categoryId" | "categorySlug" | "isASDClient" | "isNewAuto" | "isPersonalAuto" | "isShop" | "itemID" | "locationId" | "microCategoryId" | "userAuth" | "vertical" | "vehicle_type" | "withDelivery"  => {
                                    // skip
                                },
                                _ => {
                                    error!("{} :: {}.{}: {:?}", url, key, sub_key, val);
                                },
                            }
                        }
                    },
                    _ => {
                        error!("{} :: {}: {}", url, key, val);
                        map.insert(key.to_owned(), val.clone());
                    },
                }
            }
            Some(Value::Object(map))
        }
    };

    trace!("json: {}", serde_json::to_string_pretty(&json)?);
    Ok(Ret {
        client: arg.client,
        id: arg.id, 
        json,
    })
}

