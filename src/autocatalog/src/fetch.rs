
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use url::Url;
use http::StatusCode;
use serde_json::{Value, Number};

use regex::Regex;

type Item = String;

pub struct Arg {
    pub client: reqwest::Client,
    pub auth: String,
    pub item: Item,
    pub retry_count: usize,
}

use super::card::{Card, Record};

pub struct Ret {
    pub client: reqwest::Client,
    pub item: Item,
    pub card: Card,
}

const SLEEP_TIMEOUT: u64 = 500;

use std::{thread, time};

pub async fn run(arg: Arg) -> Result<Ret> {
    let url = &format!("https://avito.ru{}", 
        &arg.item,
    );
    let url = Url::parse(&url)?;

    let text = {
        let text: Result<Option<String>>;
        let mut remained = arg.retry_count;
        loop {
            let response = arg.client.get(url.clone()).send().await;
            match response {
                Err(err) => {
                    if remained > 0 {
                        remained -= 1;
                        let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
                        thread::sleep(duration);
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
                                        let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
                                        thread::sleep(duration);
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
                            return Ok(Ret {
                                client: arg.client,
                                item: arg.item, 
                                card: Card::NotFound,
                            })
                        },
                        code @ _ => {
                            if remained > 0 {
                                remained -= 1;
                                let duration = time::Duration::from_millis(SLEEP_TIMEOUT);
                                thread::sleep(duration);
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

    let card = card(arg.item.to_owned(), url, text)?;

    Ok(Ret {
        client: arg.client,
        item: arg.item, 
        card,
    })
}

fn card(id: Item, url: Url, text: Option<String>) -> Result<Card> {
    let mut power_windows: Option<String> = None;
    let mut name: Option<String> = None;
    let mut canonical_url: Option<String> = None;
    let mut item_price: Option<u64> = None;
    let mut market_price: Option<u64> = None;
    let mut fuel_type: Option<String> = None;
    let mut power_steering: Option<String> = None;
    let mut audio_system: Option<String> = None;
    let mut headlights: Option<String> = None;
    let mut production_date: Option<u16> = None;
    let mut climate_control: Option<String> = None;
    let mut interior: Option<String> = None;
    let mut vehicle_transmission: Option<String> = None;
    let mut body_type: Option<String> = None;
    let mut brand: Option<String> = None;
    let mut engine_displacement: Option<String> = None;
    let mut type_of_trade: Option<String> = None;
    let mut color: Option<String> = None;
    let mut engine_power: Option<String> = None;

    let mut condition: Option<String> = None;
    let mut description: Option<String> = None;
    let mut rims: Option<String> = None;
    let mut number_of_doors: Option<u8> = None;
    let mut owners: Option<String> = None;
    let mut autocatalog_url: Option<String> = None;
    let mut drive: Option<String> = None;
    let mut steering_wheel: Option<String> = None;
    let mut mileage: Option<u64> = None;
    let mut status: Option<String> = None;
    let mut closing_reason: Option<String> = None;
    match text {
        None => {
            error!("no text");
            Ok(Card::NoText)
        },
        Some(text) => {

            let json: Value = serde_json::from_str(&text).map_err(|_| anyhow!("{} :: failed to parse json from: {}", url, text))?;
            let map = match &json {
                Value::Object(map) => map,
                val @ _ => bail!("{} :: response json expected to be an Object, not {:?}", url, val),
            };

            for (key, val) in map.iter() {
                match key.as_str() {
                    "address" | 
                    "adjustParams" | 
                    "categoryId" | 
                    "coords" | 
                    "districtId" | 
                    "geoReferences" | 
                    "locationId" | 
                    "metroId" | 
                    "metroType" | 
                    "time" | 
                    "userType" | 
                    "titleGenerated" | 
                    "title" | 
                    "stats" | 
                    "sharing" | 
                    "seller" | 
                    "needToCheckCreditInfo" | 
                    "needToCheckSimilarItems" | 
                    "images" | 
                    "video" | 
                    "icebreakers" | 
                    "contacts" | 
                    "autotekaTeaser" | 
                    "autoCatalogAction" | 
                    "anonymousNumber" | 
                    "vehicleType" | 
                    "parameters" | 
                    "shouldLogAction" | 
                    "price" | 
                    "similarAction" | 
                    "shopId" | 
                    "shopType" | 
                    "id"  => {
                        // skip
                    },
                    "status" => {
                        let val = match val {
                            Value::String(s) => s,
                            val @ _ => bail!("{} :: '{}' expected to be an String, not {:?}", url, key, val),
                        };
                        status = Some(val.as_str().to_owned());
                    },
                    "closingReason" => {
                        let val = match val {
                            Value::String(s) => s,
                            val @ _ => bail!("{} :: '{}' expected to be an String, not {:?}", url, key, val),
                        };
                        closing_reason = Some(val.as_str().to_owned());
                    },
                    "autoCatalogUrl" => {
                        let val = match val {
                            Value::String(s) => s,
                            val @ _ => bail!("{} :: '{}' expected to be an String, not {:?}", url, key, val),
                        };
                        autocatalog_url  = Some(val.as_str().to_owned());
                    },
                    "description" => {
                        let val = match val {
                            Value::String(s) => s,
                            val @ _ => bail!("{} :: '{}' expected to be an String, not {:?}", url, key, val),
                        };
                        match &description {
                            None => {
                                description = Some(val.to_owned());
                            },
                            Some(description) => {
                                if description != val {
                                    error!("{} :: '{}' expected to be a {}, not {:?}", url, key, description, val );
                                }
                            },
                        }
                    },
                    "features" => {
                        match val {
                            Value::Null => {},
                            val @ _ => {
                                error!("{} :: {}: {}", url, key, val);
                            },
                        }
                    },
                    "priceBadge" => {
                        let sub_key = "marketPrice";
                        match val.get(sub_key) {
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
                                let val = 
                                    if val.is_u64() { 
                                        val.as_u64().unwrap() 
                                    } else if val.is_f64() { 
                                        val.as_f64().unwrap() as u64 
                                    } else { 
                                        unreachable!() 
                                    };
                                market_price = Some(val);
                            },
                        }
                    },
                    "seo" => {
                        let sub_key = "canonicalUrl";
                        match map.get(key).and_then(|val| val.get(sub_key)) {
                            None => {
                                error!("{} :: {}.{}: could not obtain", url, key, sub_key);
                            },
                            Some(val) => {
                                let val = match val {
                                    Value::String(s) => s,
                                    val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                };
                                canonical_url = Some(val.to_owned());
                            },
                        }
                    },
                    "firebaseParams" => {
                        let firebase_params = match val {
                            Value::Object(map) => map,
                            val @ _ => bail!("{} :: '{:?}' expected to be an Object, not {:?}", url, key, val),
                        };
                        for (sub_key, val) in firebase_params.iter() {
                            match sub_key.as_str() {
                                "categoryId" | 
                                "categorySlug" | 
                                "isASDClient" | 
                                "isNewAuto" | 
                                "isPersonalAuto" | 
                                "isShop" | 
                                "itemID" | 
                                "locationId" | 
                                "microCategoryId" | 
                                "userAuth" | 
                                "vertical" | 
                                "vehicle_type" | 
                                "withDelivery"  => {
                                    // skip 
                                },
                                "body_type" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
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
                                    body_type = Some(val.to_owned());
                                },
                                "brand" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    brand = Some(val.as_str().to_owned());
                                },
                                "color" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    color = Some(val.as_str().to_owned());
                                },
                                "mileage" => {
                                    let val = match val {
                                        Value::String(s) => {
                                            lazy_static! {
                                                static ref RE_MATCH: Regex = Regex::new(r"(?:\d+)(?:\s+(?:\d)+)*(:?\s*км)?").unwrap();
                                                static ref RE_REPLACE: Regex = Regex::new(r"\D").unwrap();
                                            }
                                            if !RE_MATCH.is_match(s) {
                                                bail!("{} :: '{}.{}' expected to match 'DDD DDD км, but: {}", url, key, sub_key, val);
                                            }
                                            let s = RE_REPLACE.replace_all(s, "");
                                            Value::Number(
                                                Number::from_f64(
                                                    s.parse::<u64>()
                                                        .map_err(|err| Error::new(err))
                                                        .context(format!("{} :: '{}.{}' expected to be a String parsable to Number, not {:?} <= {:?}", url, key, sub_key, s, val))? as f64
                                                ).unwrap()
                                            )
                                        },
                                        Value::Number(n) => Value::Number(n.clone()),
                                        val @ _ => bail!("{} :: '{}.{}' expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                    };
                                    let val = 
                                        if val.is_u64() { 
                                            val.as_u64().unwrap() 
                                        } else if val.is_f64() { 
                                            val.as_f64().unwrap() as u64 
                                        } else { unreachable!() };
                                    mileage = Some(val);
                                },
                                "capacity" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    engine_power = Some(val.as_str().to_owned());
                                },
                                "engine" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    engine_displacement = Some(val.as_str().to_owned());
                                }, 
                                "type_of_trade" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    type_of_trade = Some(val.as_str().to_owned());
                                },
                                "condition" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    condition = Some(val.as_str().to_owned());
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
                                    drive = Some(val.to_owned());
                                },
                                "engine_type" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!(" {} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    fuel_type = Some(val.as_str().to_owned());
                                },
                                "model" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    name = Some(val.as_str().to_owned());
                                },
                                "transmission" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
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
                                    vehicle_transmission = Some(val.to_owned());
                                },
                                "vladeltsev_po_pts" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    owners = Some(val.as_str().to_owned());
                                },
                                "audiosistema" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    audio_system = Some(val.as_str().to_owned());
                                },
                                "wheel" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    steering_wheel = Some(val.as_str().to_owned());
                                },
                                "elektrosteklopodemniki" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    power_windows = Some(val.as_str().to_owned());
                                },
                                "usilitel_rulya" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    power_steering = Some(val.as_str().to_owned());
                                },
                                "diski" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    rims = Some(val.as_str().to_owned());
                                },
                                "salon" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    interior = Some(val.as_str().to_owned());
                                },
                                "upravlenie_klimatom" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    climate_control = Some(val.as_str().to_owned());
                                },
                                "fary" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    headlights = Some(val.as_str().to_owned());
                                },
                                "itemPrice" => {
                                    let val = match val {
                                        Value::String(s) => Value::Number(
                                            Number::from_f64(
                                                s.parse::<u64>()
                                                    .map_err(|err| Error::new(err))
                                                    .context(format!("{} :: '{}.{}' expected to be a String parsable to Number, not {:?}", url, key, sub_key, s))? as f64
                                            ).unwrap()
                                        ),
                                        Value::Number(n) => Value::Number(n.clone()),
                                        val @ _ => bail!("{} :: '{}.{}' expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                    };
                                    let val = 
                                        if val.is_u64() { 
                                            val.as_u64().unwrap() 
                                        } else if val.is_f64() { 
                                            val.as_f64().unwrap() as u64 
                                        } else { unreachable!() };
                                    match item_price {
                                        None => item_price = Some(val),
                                        Some(item_price) => {
                                            if item_price != val {
                                                error!("{} :: '{}.{}' expected to be a {}, not {:?}", url, key, sub_key, item_price, val );
                                            }
                                        },
                                    }
                                },
                                "year" => {
                                    let val = match val {
                                        Value::String(s) => {
                                             match s.parse::<u16>() {
                                                Err(err) => {
                                                    let card = Card::WithError {
                                                        json: json.to_owned(),
                                                        error: format!("{} :: '{}.{}' expected to be a String parsable to u16, not {:?}: {:?} ", url, key, sub_key, s, err),
                                                    };
                                                    return Ok(card);
                                                },
                                                Ok(n) => n as u16,
                                            }
                                        },
                                        Value::Number(val) => {
                                            if val.is_u64() { 
                                                val.as_u64().unwrap() as u16
                                            } else if val.is_f64() { 
                                                val.as_f64().unwrap() as u16 
                                            } else { 
                                                unreachable!() 
                                            }
                                        }
                                        val @ _ => bail!("{} :: '{}.{}' expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                    };
                                    production_date = Some(val);
                                },
                                "kolichestvo_dverey" => {
                                    let val = match val {
                                        Value::String(s) => Value::Number(
                                            Number::from_f64(
                                                s.parse::<u64>()
                                                    .map_err(|err| Error::new(err))
                                                    .context(format!("{} :: '{}.{}' expected to be a String parsable to Number, not {:?}", url, key, sub_key, s))? as f64
                                            ).unwrap()
                                        ),
                                        Value::Number(n) => Value::Number(n.clone()),
                                        val @ _ => bail!("{} :: '{}.{}' expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                    };
                                    let val = 
                                        if val.is_u64() { 
                                            val.as_u64().unwrap() as u8
                                        } else if val.is_f64() { 
                                            val.as_f64().unwrap() as u64 as u8
                                        } else { unreachable!() };
                                    number_of_doors = Some(val);
 
                                },
                                "description" => {
                                    let val = match val {
                                        Value::String(s) => s,
                                        val @ _ => bail!("{} :: '{}.{}' expected to be an String, not {:?}", url, key, sub_key, val),
                                    };
                                    match &description {
                                        None => {
                                            description = Some(val.as_str().to_owned());
                                        },
                                        Some(description) => {
                                            if description != val {
                                                error!("{} :: '{}.{}' expected to be a {}, not {:?}", url, key, sub_key, description, val );
                                            }
                                        },
                                    }
                                },
                                "price" => {
                                    let val = match val {
                                        Value::String(s) => Value::Number(
                                            Number::from_f64(
                                                s.parse::<u64>()
                                                    .map_err(|err| Error::new(err))
                                                    .context(format!("{} :: '{}.{}' expected to be a String parsable to Number, not {:?}", url, key, sub_key, s))? as f64
                                            ).unwrap()
                                        ),
                                        Value::Number(n) => Value::Number(n.clone()),
                                        val @ _ => bail!("{} :: '{}.{}' expected to be a Number or a String, not {:?}", url, key, sub_key, val),
                                    };
                                    let val = 
                                        if val.is_u64() { 
                                            val.as_u64().unwrap() 
                                        } else if val.is_f64() { 
                                            val.as_f64().unwrap() as u64 
                                        } else { unreachable!() };
                                    match item_price {
                                        None => item_price = Some(val),
                                        Some(item_price) => {
                                            if item_price != val {
                                                error!("{} :: '{}.{}' expected to be a {}, not {:?}", url, key, sub_key, item_price, val );
                                            }
                                        },
                                    }
                                },
                                _ => {
                                    error!("{} :: {}.{}: {:?}", url, key, sub_key, val);
                                    unreachable!();
                                },
                            }
                        }
                    },
                    _ => {
                        error!("{} :: {}: {}", url, key, val);
                        unreachable!();
                    },
                }
            }
            let record = Record {
                avito_id: id,
                power_windows,
                name,
                canonical_url,
                item_price,
                market_price,
                fuel_type,
                power_steering,
                audio_system,
                headlights,
                production_date,
                climate_control,
                interior,
                vehicle_transmission,
                body_type,
                brand,
                engine_displacement,
                type_of_trade,
                color,
                engine_power,
                condition,
                description,
                rims,
                number_of_doors,
                owners,
                autocatalog_url,
                drive,
                steering_wheel,
                mileage,
                status,
                closing_reason,
            };
            Ok(Card::Record (record))
        }
    }
}

