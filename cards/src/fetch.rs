
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use url::Url;
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
    pub json: Value,
}

pub async fn run(arg: Arg) -> Result<Ret> {
    let url = &format!("https://avito.ru/api/14/items/{}?key={}", 
        arg.id,
        arg.auth, 
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
    let json = match json {
        Value::Object(map) => map,
        val @ _ => bail!("response json expected to be an Object, not {:?}", val),
    };

    let mut map = Map::new();
    for (key, val) in json.iter() {
        match key.as_str() {
            "address" | "adjustParams" | "categoryId" | "coords" | "districtId" | "geoReferences" | "locationId" | "metroId" | "metroType" | "time" | "userType" | "titleGenerated" | "title" | "stats" | "sharing" | "seo" | "seller" | "needToCheckCreditInfo" | "needToCheckSimilarItems" | "images" | "video" | "icebreakers" | "contacts" | "autotekaTeaser" | "autoCatalogAction" | "anonymousNumber" | "vehicleType" | "parameters" | "shouldLogAction" | "price" | "similarAction" => {
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
                match json.get(key).and_then(|val| val.get("marketPrice")) {
                    None => {
                        error!("could not obtain priceBadge.marketPrice");
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
                                            .context(format!("response json 'priceBadge.marketPrice' expected to be a String parsable to Number, not {:?}", s))? as f64
                                    ).unwrap()
                                )
                            },
                            Value::Number(n) => Value::Number(n.clone()),
                            val @ _ => bail!("response json 'priceBadge.marketPrice' expected to be a Number or a String, not {:?}", val),
                        };
                        map.insert("marketPrice".to_owned(), val.clone());
                    },
                }
            },
            "firebaseParams" => {
                let firebase_params = match val {
                    Value::Object(map) => map,
                    val @ _ => bail!("response json '{:?}' expected to be an Object, not {:?}", key, val),
                };
                for (key, val) in firebase_params.iter() {
                    match key.as_str() {
                        "body_type" => {
                            map.insert("bodyType".to_owned(), val.clone());
                            let val = match val {
                                Value::String(s) => s,
                                val @ _ => bail!("response json 'firebaseParams.{}' expected to be an String, not {:?}", key, val),
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
                                    error!("firebaseParams.{}: {:?}", key, s);
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
                                val @ _ => bail!("response json 'firebaseParams.{}' expected to be an String, not {:?}", key, val),
                            };
                            let val = match val.as_str() {
                                "Механика" => "механическая",
                                "Автомат" => "автоматическая",
                                "Вариатор" => "вариатор",
                                "Робот" => "робот",
                                s @ _ => {
                                    error!("firebaseParams.{}: {}", key, s);
                                    s
                                }
                            };
                            map.insert("vehicleTransmission".to_owned(), Value::String(val.to_owned()));
                        },
                        "drive" => {
                            let val = match val {
                                Value::String(s) => s,
                                val @ _ => bail!("response json 'firebaseParams.drive' expected to be an String, not {:?}", val),
                            };
                            let val = match val.as_str() {
                                "Передний" => "передний",
                                "Задний" => "задний",
                                "Полный" => "полный",
                                s @ _ => {
                                    error!("firebaseParams.drive: {}", s);
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
                        "itemPrice" | "kolichestvo_dverey" 
                            // | "vladeltsev_po_pts" 
                            => {
                            let val = match val {
                                Value::String(s) => Value::Number(
                                    Number::from_f64(
                                        s.parse::<u64>()
                                            .map_err(|err| Error::new(err))
                                            .context(format!("response json 'firebaseParams.{}' expected to be a String parsable to Number, not {:?}", key, s))? as f64
                                    ).unwrap()
                                ),
                                Value::Number(n) => Value::Number(n.clone()),
                                val @ _ => bail!("response json 'firebaseParams.{}' expected to be a Number or a String, not {:?}", key, val),
                            };
                            let key_new = match key.as_str() {
                                "kolichestvo_dverey" => "numberOfDoors",
                                // "vladeltsev_po_pts" => "Владельцы",
                                "itemPrice" => "itemPrice",
                                _ => unreachable!(),

                            };
                            map.insert(key_new.to_owned(), val);
                        },
                        "brand" | "color" | "mileage" | "capacity" | "drive" | "engine" | "itemPrice" | "type_of_trade" 
 => {

                                    map.insert(key.to_owned(), val.clone());
                        },
                        "categoryId" | "categorySlug" | "isASDClient" | "isNewAuto" | "isPersonalAuto" | "isShop" | "itemID" | "locationId" | "microCategoryId" | "userAuth" | "vertical" | "vehicle_type" | "withDelivery"  => {
                            // skip
                        },
                        _ => {
                            error!("firebaseParams.{}: {:?}", key, val);
                        },
                    }
                }
            },
            _ => {
                error!("{}: {}", key, val);
                map.insert(key.to_owned(), val.clone());
            },
        }
    }
    let json = Value::Object(map);

    trace!("json: {}", serde_json::to_string_pretty(&json)?);
    Ok(Ret {
        client: arg.client,
        id: arg.id, 
        json,
    })
}

