
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use serde::{
    Serialize, 
    Deserialize,
};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub enum Card {
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
    pub id: u64,
    pub title: Option<String>,
    #[serde(rename = "Коробка передач")]
    pub transmission: Option<String>,
    #[serde(rename = "Мощность, л.с.")]
    pub engine_power: Option<u16>,
    #[serde(rename = "Объем двигателя, л")]
    pub engine_displacement: Option<f64>,
    #[serde(rename = "Привод")]
    pub drive: Option<String>,
    #[serde(rename = "Разгон до 100 км/ч, с")]
    pub acceleration: Option<String>,
    #[serde(rename = "Расход топлива смешанный, л/100 км")]
    pub fuel_consumption_mixed: Option<String>,
    #[serde(rename = "Тип двигателя")]
    pub fuel_type: Option<String>,
    #[serde(rename = "Страна происхождения бренда")]
    pub brand_country: Option<String>,
    #[serde(rename = "Количество мест")]
    pub number_of_seats: Option<u8>,
    #[serde(rename = "Рейтинг EuroNCAP")]
    pub rating: Option<String>,
    #[serde(rename = "Страна сборки")]
    pub assembly_country: Option<String>,
    #[serde(rename = "Количество цилиндров")]
    pub number_of_cylinders: Option<String>,
    #[serde(rename = "Конфигурация")]
    pub configuration: Option<String>,
    #[serde(rename = "Крутящий момент, Н⋅м")]
    pub torque: Option<u16>,
    #[serde(rename = "Обороты максимального крутящего момента, об/мин")]
    pub torque_max: Option<u16>,
    #[serde(rename = "Обороты максимальной мощности, об/мин")]
    pub max_power_speed: Option<u16>,
    #[serde(rename = "Рабочий объем, см³")]
    pub engine_displacement_precise: Option<u16>,
    #[serde(rename = "Высота, мм")]
    pub height: Option<u16>,
    #[serde(rename = "Диаметр разворота, м")]
    pub turning_diameter: Option<u8>,
    #[serde(rename = "Длина, мм")]
    pub length: Option<u16>,
    #[serde(rename = "Дорожный просвет, мм")]
    pub clearance: Option<u16>,
    #[serde(rename = "Колесная база, мм")]
    pub wheelbase: Option<u16>,
    #[serde(rename = "Колея задняя, мм")]
    pub rear_track: Option<u16>,
    #[serde(rename = "Колея передняя, мм")]
    pub front_track: Option<u16>,
    #[serde(rename = "Объем багажника, л")]
    pub trunk_volume: Option<u16>,
    #[serde(rename = "Емкость топливного бака, л")]
    pub fuel_tank_capacity: Option<u16>,
    #[serde(rename = "Максимальная скорость, км/ч")]
    pub maximum_speed: Option<u16>,
    #[serde(rename = "Расход топлива в городе, л/100 км")]
    pub fuel_consumption_city: Option<u16>,
    #[serde(rename = "Расход топлива по трассе, л/100 км")]
    pub fuel_consumption_highway: Option<u16>,
    #[serde(rename = "Экологический класс")]
    pub environmental_class: Option<String>,
    #[serde(rename = "Задние тормоза")]
    pub rear_breaks: Option<String>,
    #[serde(rename = "Передние тормоза")]
    pub front_breaks: Option<String>,
    #[serde(rename = "Размерность задних шин")]
    pub rear_tire_dimension: Option<String>,
    #[serde(rename = "Размерность передних шин")]
    pub front_tire_dimension: Option<String>,
}

use url::Url;
use regex::Regex;

// use tokio::fs::File;
// use tokio::prelude::*;
// use std::path::Path;
// use std::fs::File;
// use std::io::prelude::*;

type Item = str;
impl Card {
    pub fn parse<I: AsRef<Item>, S: AsRef<str>>(text: S, url: Url, item: I) -> Result<Card> {
        lazy_static! {
            static ref RE_MATCH: Regex = Regex::new(r#"data-autocatalog="([^"]+)""#).unwrap();
            static ref RE_REPLACE: Regex = Regex::new(r#"&quot;"#).unwrap();
            static ref RE_REMOVE: Regex = Regex::new(r"\ufeff").unwrap();
            // static ref RE_MATCH_START: Regex = Regex::new(r#"\{"id":"#).unwrap();
            // static ref RE_MATCH_END: Regex = Regex::new(r#"\]\},"defaultModificationId"#).unwrap();
        }
        let mut ids: Vec<u64> = vec![];
        match RE_MATCH.captures(text.as_ref()) {
            None => { 
                unreachable!() 
            },
            Some(caps) => {
                let data_autocatalog = caps.get(1).unwrap().as_str();
                let decoded = RE_REPLACE.replace_all(data_autocatalog, r#"""#);
                let decoded = RE_REMOVE.replace_all(&decoded, "");
                let json: Value = serde_json::from_str(&decoded)?;
                Self::parse_json(&json, &url, item.as_ref())?;
                todo!();
                // info!("json: {}", );
                // let file_path = Path::new("test_data/autocatalog_data.json");
                // let mut file = File::create(file_path)?;
                // file.write_all(serde_json::to_string_pretty(&json).unwrap().as_bytes())?;
                //
                // let mut id_start_opt: Option<usize> = None;
                // let mut pos = 0;
                // loop {
                //     match RE_MATCH_START.find_at(&decoded, pos) {
                //         Some(start) => {
                //             info!("start: start: {}, end: {}", start.start(), start.end());
                //             match id_start_opt {
                //                 None => {
                //                     id_start_opt = Some(start.start());
                //                 },
                //                 Some(id_start) => {
                //                     let s = &decoded[id_start..start.start() - 1];
                //                     let id = Self::parse_helper(s, &url)?;
                //                     ids.push(id);
                //                     id_start_opt = Some(start.start());
                //                 },
                //             }
                //             pos = start.end();
                //         },
                //         None => {
                //             if let Some(id_start) = id_start_opt {
                //                 match RE_MATCH_END.find_at(&decoded, pos) {
                //                     None => { unreachable!(); },
                //                     Some(end) => {
                //
                //                         let s = &decoded[id_start..end.start()];
                //                         let id = Self::parse_helper(s, &url)?;
                //                         ids.push(id);
                //                         // id_start_opt = Some(start.start());
                //
                //                         // info!("end: start: {}, end: {}", end.start(), end.end());
                //                         break;
                //                     },
                //                 }
                //             }
                //         },
                //     }
                // };
            },
        }
        // info!("ids: {:?}", ids);
        // todo!();
    }
    fn parse_json(json: &Value, url: &Url, item: &str) -> Result<Vec<Card>> {
        let key = "modifications";
        let sub_key = "items";
        let items: &Vec<Value> = match json.get(key).and_then(|val| val.get(sub_key)) {
            None => {
                bail!("{}::{}.{}: could not obtain from {:?}", url, key, sub_key, json);
            },
            Some(val) => {

                match val {
                    Value::Array(val) =>  val,
                    val @ _ => bail!("{}::'{}.{}' expected to be {}, not {:?}", url, key, sub_key, "an Array", val),
                }

            },
        };
        let mut ret: Vec<Card> = vec![];
        // for item in items {
        //     let id = 
        // }
        info!("items.len: {}", items.len());
        todo!();

    //     let key = "id";
    //     let id = match json.get("id") {
    //         None => {
    //             bail!("{} :: {}: could not obtain from {:?}", url, key, json);
    //         },
    //         Some(val) => {
    //             match val {
    //                 Value::Number(val) => {
    //                     if val.is_u64() { 
    //                         val.as_u64().unwrap()
    //                     } else { 
    //                         unreachable!() 
    //                     }
    //                 },
    //                 val @ _ => bail!("{} :: '{}' expected to be an Number, not {:?}", url, key, val),
    //             }
    //         },
    //     };
    }
    // fn parse_helper(s: &str, url: &Url) -> Result<u64> {
    //                                 // let map = match &json {
    //                                 //     Value::Object(map) => map,
    //                                 //     val @ _ => bail!("{} :: json expected to be an Object, not {:?}", url, json),
    //                                 // };
    //                                 
    //                                 // info!("json: {}", serde_json::to_string_pretty(&json)?);
    //                                 // todo!();
    //     let json: Value = serde_json::from_str(s).context(format!(r#"{}: failed to decode json from data-autocatalog="{}""#, url, s))?;
    //
    //     let key = "id";
    //     let id = match json.get("id") {
    //         None => {
    //             bail!("{} :: {}: could not obtain from {:?}", url, key, json);
    //         },
    //         Some(val) => {
    //             match val {
    //                 Value::Number(val) => {
    //                     if val.is_u64() { 
    //                         val.as_u64().unwrap()
    //                     } else { 
    //                         unreachable!() 
    //                     }
    //                 },
    //                 val @ _ => bail!("{} :: '{}' expected to be an Number, not {:?}", url, key, val),
    //             }
    //         },
    //     };
    //     Ok(id)
    // }
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

    use tokio::fs::File;
    use tokio::prelude::*;

    #[tokio::test]
    async fn test_parse_json() -> Result<()> {
        init();

        let mut file = File::open("test_data/autocatalog.json").await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let decoded = std::str::from_utf8(&contents)?;

        let json: Value = serde_json::from_str(&decoded)?;
        let item = "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896";
        let url = Url::parse("https://avito.ru/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896")?;
        Card::parse_json(&json, &url, &item)?;
        // let card = Card::parse_json(&json, &url, item)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_parse() -> Result<()> {
        init();
// let s = "
//         {
//             \"fingerprint\": \"0xF9BA143B95FF6D82\",
//             \"location\": \"Menlo Park, CA\"
//         }";
//
//         let json: Value = serde_json::from_str(s)?;//.context(format!(r#"{}: failed to decode json from data-autocatalog="{}""#, url, s))?;
//         info!("json: {}", serde_json::to_string_pretty(&json)?);

        let mut file = File::open("test_data/page.html").await?;
        let mut contents = vec![];;
        file.read_to_end(&mut contents).await?;
        let text = std::str::from_utf8(&contents)?;
        let item = "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896";
        let url = Url::parse("https://avito.ru/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896")?;
        let card = Card::parse(text, url, item)?;


        // let client = reqwest::Client::new();
        // let arg = Arg {
        //     client,
        //     item: "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896",
        //     retry_count: 3,
        // };
        //
        // let _ret = run(arg).await?;
        Ok(())
    }
}
