
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use serde::{
    Serialize, 
    Deserialize,
};
use serde_json::Value;
use regex::Regex;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub enum Fetched {
    NotFound,
    NoText,
    WithError {
        json: Value,
        error: String,
    },
    Record(Record)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub body_type: Option<String>,
    pub brand: Option<String>,
    pub color: Option<String>,
    pub fuel_type: Option<String>,
    // modelDate - год модели
    pub name: Option<String>,
    pub title: Option<String>, // модель автомобиля по автокаталогу
    pub number_of_doors: Option<u8>,
    pub production_date: Option<u16>,
    // vehicle_configuration
    pub vehicle_transmission: Option<String>,
    pub engine_displacement: Option<String>,
    pub engine_power: Option<String>,
    pub description: Option<String>,
    pub mileage: Option<u64>,
    // Комлектация
    #[serde(rename = "Привод")]
    pub drive: Option<String>,
    #[serde(rename = "Руль")]
    pub steering_wheel: Option<String>,
    #[serde(rename = "Состояние")]
    pub condition: Option<String>,
    #[serde(rename = "Владельцы")]
    pub owners: Option<String>,
    // ПТС
    // Таможня
    // Владение
    // id

    #[serde(rename = "Электростеклоподъемники")]
    pub power_windows: Option<String>,
    #[serde(rename = "Усилитель руля")]
    pub power_steering: Option<String>,
    #[serde(rename = "Аудиосистема")]
    pub audio_system: Option<String>,
    #[serde(rename = "Фары")]
    pub headlights: Option<String>,
    #[serde(rename = "Климат-контроль")]
    pub climate_control: Option<String>,
    #[serde(rename = "Салон")]
    pub interior: Option<String>,
    #[serde(rename = "Диски")]
    pub rims: Option<String>,

    #[serde(rename = "autoCatalogUrl")]
    pub autocatalog_url: Option<String>,

    pub item_price: Option<u64>,
    pub market_price: Option<u64>,

    pub status: Option<String>,
    pub closing_reason: Option<String>,
    pub complectation: Option<String>,
    pub modification: Option<String>,
    pub generation: Option<String>,
    pub type_of_trade: Option<String>,
    pub canonical_url: Option<String>,

    // #[serde(flatten)]
    // pub autocatalog: Option<super::autocatalog::Record>,

    pub autocatalog_id: Option<u64>,
    pub autocatalog_title: Option<String>,

    // #[serde(rename = "Коробка передач")]
    pub autocatalog_transmission: Option<String>,

    // #[serde(rename = "Объем двигателя, л")]
    pub autocatalog_engine_displacement: Option<String>,
    // #[serde(rename = "Рабочий объем, см³")]
    pub autocatalog_engine_displacement_precise: Option<String>,

    // #[serde(rename = "Привод")]
    pub autocatalog_drive: Option<String>,
    // #[serde(rename = "Тип двигателя")]
    pub autocatalog_fuel_type: Option<String>,

    // #[serde(rename = "Мощность, л.с.")]
    pub autocatalog_engine_power: Option<String>,
    // #[serde(rename = "Максимальная скорость, км/ч")]
    pub autocatalog_maximum_speed: Option<String>,
    // #[serde(rename = "Разгон до 100 км/ч, с")]
    pub autocatalog_acceleration: Option<String>,

    // #[serde(rename = "Страна происхождения бренда")]
    pub autocatalog_brand_country: Option<String>,
    // #[serde(rename = "Страна сборки")]
    pub autocatalog_assembly_country: Option<String>,

    // #[serde(rename = "Количество мест")]
    pub autocatalog_number_of_seats: Option<String>,

    // #[serde(rename = "Рейтинг EuroNCAP")]
    pub autocatalog_rating: Option<String>,

    // #[serde(rename = "Количество цилиндров")]
    pub autocatalog_number_of_cylinders: Option<String>,
    // #[serde(rename = "Конфигурация")]
    pub autocatalog_configuration: Option<String>,

    // #[serde(rename = "Крутящий момент, Н⋅м")]
    pub autocatalog_torque: Option<String>,
    // #[serde(rename = "Обороты максимального крутящего момента, об/мин")]
    pub autocatalog_torque_max: Option<String>,
    // #[serde(rename = "Обороты максимальной мощности, об/мин")]
    pub autocatalog_max_power_speed: Option<String>,

    // #[serde(rename = "Высота, мм")]
    pub autocatalog_height: Option<String>,
    // #[serde(rename = "Длина, мм")]
    pub autocatalog_length: Option<String>,
    // #[serde(rename = "Диаметр разворота, м")]
    pub autocatalog_turning_diameter: Option<String>,
    // #[serde(rename = "Дорожный просвет, мм")]
    pub autocatalog_clearance: Option<String>,
    // #[serde(rename = "Колесная база, мм")]
    pub autocatalog_wheelbase: Option<String>,
    // #[serde(rename = "Колея задняя, мм")]
    pub autocatalog_rear_track: Option<String>,
    // #[serde(rename = "Колея передняя, мм")]
    pub autocatalog_front_track: Option<String>,

    // #[serde(rename = "Объем багажника, л")]
    pub autocatalog_trunk_volume: Option<String>,

    // #[serde(rename = "Емкость топливного бака, л")]
    pub autocatalog_fuel_tank_capacity: Option<String>,

    // #[serde(rename = "Расход топлива в городе, л/100 км")]
    pub autocatalog_fuel_consumption_city: Option<String>,
    // #[serde(rename = "Расход топлива по трассе, л/100 км")]
    pub autocatalog_fuel_consumption_highway: Option<String>,
    // #[serde(rename = "Расход топлива смешанный, л/100 км")]
    pub autocatalog_fuel_consumption_mixed: Option<String>,

    // #[serde(rename = "Экологический класс")]
    pub autocatalog_environmental_class: Option<String>,

    // #[serde(rename = "Задние тормоза")]
    pub autocatalog_rear_breaks: Option<String>,
    // #[serde(rename = "Передние тормоза")]
    pub autocatalog_front_breaks: Option<String>,

    // #[serde(rename = "Размерность задних шин")]
    pub autocatalog_rear_tire_dimension: Option<String>,
    // #[serde(rename = "Размерность передних шин")]
    pub autocatalog_front_tire_dimension: Option<String>,

    // #[serde(rename = "Задняя подвеска")]
    pub autocatalog_rear_suspension: Option<String>,
    // #[serde(rename = "Передняя подвеска")]
    pub autocatalog_front_suspension: Option<String>,

    // #[serde(rename = "Мировая премьера")]
    pub autocatalog_world_premier: Option<String>,
    // #[serde(rename = "Ожидаемое обновление")]
    pub autocatalog_pending_update: Option<String>,
    // #[serde(rename = "Ширина с (зеркалами), мм")]
    pub autocatalog_width_with_mirrors: Option<String>,
    // #[serde(rename = "Размерность задних дисков")]
    pub autocatalog_rear_disc_dimension: Option<String>,
    // #[serde(rename = "Размерность передних дисков")]
    pub autocatalog_front_disc_dimension: Option<String>,
}

use url::Url;
use json::{Json, By};
use tokio::prelude::*;

impl Fetched {
    pub async fn parse<S: AsRef<str>>(text: S, url: Url) -> Result<Self> {
        let json = Json::from_str(&text, url)?;
        match Self::parse_json(&json) {
            Ok(record) => Ok(Fetched::Record (record)),
            Err(err) => {
                let file_path = Path::new("error.card.json");
                let mut file = fs::File::create(file_path).await.context(format!("file_path: {:?}", file_path))?;
                file.write_all(text.as_ref().as_bytes()).await?;
                Ok(Fetched::WithError {json: json.value, error: format!("{}", err)})
            },
        }
    }
    pub fn parse_json(json: &Json) -> Result<Record> {
        let mut power_windows: Option<String> = None;
        let mut name: Option<String> = None;
        let title = None;
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
        let mut complectation: Option<String> = None;
        let mut generation: Option<String> = None;
        let mut modification: Option<String> = None;
        for (key, val) in json.iter_map()? {
            match key {
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
                "advertOptions" | 
                "id"  => {
                    // skip
                },
                "status" => {
                    status = Some(val.as_string()?);
                },
                "closingReason" => {
                    closing_reason = Some(val.as_string()?);
                },
                "autoCatalogUrl" => {
                    autocatalog_url  = Some(val.as_string()?);
                },
                "description" => {
                    let v = val.as_str()?;
                    match &description {
                        None => {
                            description = Some(v.to_owned());
                        },
                        Some(description) => {
                            if description != v {
                                warn!("{} expected to be a {}, not {:?}", val.path, description, v);
                            }
                        },
                    }
                },
                "features" => {
                    val.as_null()?;
                },
                "priceBadge" => {
                    lazy_static! {
                        static ref RE: Regex = Regex::new(r"[^\d]").unwrap();
                    }
                    let val = val.get([By::key("marketPrice")])?.parse_as_u64_after(|s| RE.replace_all(s, ""))?;
                    market_price = Some(val);

                },
                "seo" => {
                    let val = val.get([By::key("canonicalUrl")])?;
                    canonical_url = Some(val.as_string()?);
                },
                "firebaseParams" => {
                    for (sub_key, val) in val.iter_map()? {
                        match sub_key {
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
                                let s = match val.as_str()? {
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
                                        warn!("{}: {:?}", val.path, s);
                                        s
                                    }
                                };
                                body_type = Some(s.to_owned());
                            },
                            "brand" => {
                                brand = Some(val.as_string()?);
                            },
                            "color" => {
                                color = Some(val.as_string()?);
                            },
                            "mileage" => {
                                lazy_static! {
                                    static ref RE_REPLACE: Regex = Regex::new(r"\D").unwrap();
                                }
                                mileage = Some(val.parse_as_u64_after(|s| RE_REPLACE.replace_all(s, ""))?);
                            },
                            "capacity" => {
                                engine_power = Some(val.as_string()?);
                            },
                            "engine" => {
                                engine_displacement = Some(val.as_string()?);
                            }, 
                            "type_of_trade" => {
                                type_of_trade = Some(val.as_string()?);
                            },
                            "condition" => {
                                condition = Some(val.as_string()?);
                            },
                            "drive" => {
                                let s = match val.as_str()? {
                                    "Передний" => "передний",
                                    "Задний" => "задний",
                                    "Полный" => "полный",
                                    s @ _ => {
                                        warn!("{}: {}", val.path, s);
                                        s
                                    }
                                };
                                drive = Some(s.to_owned());
                            },
                            "engine_type" => {
                                fuel_type = Some(val.as_string()?);
                            },
                            "model" => {
                                name = Some(val.as_string()?);
                            },
                            "transmission" => {
                                let val = match val.as_str()? {
                                    "Механика" => "механическая",
                                    "Автомат" => "автоматическая",
                                    "Вариатор" => "вариатор",
                                    "Робот" => "робот",
                                    s @ _ => {
                                        warn!("{}: {}", val.path, s);
                                        s
                                    }
                                };
                                vehicle_transmission = Some(val.to_owned());
                            },
                            "vladeltsev_po_pts" => {
                                owners = Some(val.as_string()?);
                            },
                            "audiosistema" => {
                                audio_system = Some(val.as_string()?);
                            },
                            "wheel" => {
                                steering_wheel = Some(val.as_string()?);
                            },
                            "elektrosteklopodemniki" => {
                                power_windows = Some(val.as_string()?);
                            },
                            "usilitel_rulya" => {
                                power_steering = Some(val.as_string()?);
                            },
                            "diski" => {
                                rims = Some(val.as_string()?);
                            },
                            "salon" => {
                                interior = Some(val.as_string()?);
                            },
                            "upravlenie_klimatom" => {
                                climate_control = Some(val.as_string()?);
                            },
                            "fary" => {
                                headlights = Some(val.as_string()?);
                            },
                            "itemPrice" => {
                                let v = val.parse_as_u64()?;
                                match item_price {
                                    None => item_price = Some(v),
                                    Some(item_price) => {
                                        if item_price != v {
                                            warn!("{} expected to be a {}, not {:?}", val.path, item_price, v );
                                        }
                                    },
                                }
                            },
                            "year" => {
                                production_date = Some(val.parse_as_u16()?);
                            },
                            "kolichestvo_dverey" => {
                                number_of_doors = Some(val.parse_as_u8()?);
                            },
                            "description" => {
                                let v = val.as_str()?;
                                match &description {
                                    None => {
                                        description = Some(v.to_owned());
                                    },
                                    Some(description) => {
                                        if description != v {
                                            warn!("{} expected to be a {}, not {:?}", val.path, description, v );
                                        }
                                    },
                                }
                            },
                            "price" => {
                                let v = val.parse_as_u64()?;
                                match item_price {
                                    None => item_price = Some(v),
                                    Some(item_price) => {
                                        if item_price != v {

                                            warn!("{} expected to be a {}, not {:?}", val.path, item_price, v );

                                        }
                                    },
                                }
                            },
                            "complectation" => {
                                complectation = Some(val.as_string()?);
                            },
                            "generation" => {
                                generation = Some(val.as_string()?);
                            },
                            "modification" => {
                                modification = Some(val.as_string()?);
                            },
                            _ => {
                                bail!("unexpected {}: {}", val.path, val.value);
                            },
                        }
                    }
                },
                _ => {
                    bail!("unexpected {}: {}", val.path, val.value);
                },
            }
        }
        Ok(Record {
            power_windows,
            name,
            title,
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
            complectation,
            generation,
            modification,

            autocatalog_id: None,
            autocatalog_title: None,

            autocatalog_transmission: None,

            autocatalog_engine_displacement: None,
            autocatalog_engine_displacement_precise: None,

            autocatalog_drive: None,
            autocatalog_fuel_type: None,

            autocatalog_engine_power: None,
            autocatalog_maximum_speed: None,
            autocatalog_acceleration: None,

            autocatalog_brand_country: None,
            autocatalog_assembly_country: None,

            autocatalog_number_of_seats: None,

            autocatalog_rating: None,

            autocatalog_number_of_cylinders: None,
            autocatalog_configuration: None,

            autocatalog_torque: None,
            autocatalog_torque_max: None,
            autocatalog_max_power_speed: None,

            autocatalog_height: None,
            autocatalog_length: None,
            autocatalog_turning_diameter: None,
            autocatalog_clearance: None,
            autocatalog_wheelbase: None,
            autocatalog_rear_track: None,
            autocatalog_front_track: None,

            autocatalog_trunk_volume: None,

            autocatalog_fuel_tank_capacity: None,

            autocatalog_fuel_consumption_city: None,
            autocatalog_fuel_consumption_highway: None,
            autocatalog_fuel_consumption_mixed: None,

            autocatalog_environmental_class: None,

            autocatalog_rear_breaks: None,
            autocatalog_front_breaks: None,

            autocatalog_rear_tire_dimension: None,
            autocatalog_front_tire_dimension: None,

            autocatalog_rear_suspension: None,
            autocatalog_front_suspension: None,

            autocatalog_world_premier: None,
            autocatalog_pending_update: None,
            autocatalog_width_with_mirrors: None,
            autocatalog_rear_disc_dimension: None,
            autocatalog_front_disc_dimension: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;

    use tokio::fs::File;

    use std::path::Path;
    #[tokio::test]
    async fn test_parse_json() -> Result<()> {
        test_helper::init();

        let file_path = Path::new("/home/rust/src/out/error.card.txt");
        info!("here");
        let mut file = File::open(file_path).await.context(format!("file_path: {:?}", file_path))?;
        info!("there");
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let s = std::str::from_utf8(&contents)?;
        let _ret = Fetched::parse(s.to_owned(), Url::parse("https://sob.ru")?).await?;
        // info!("ret: {:#?}", ret);

        Ok(())
    }
}
