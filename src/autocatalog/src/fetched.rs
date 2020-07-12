
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use json::{Json, By};

use serde::{
    Serialize, 
    Deserialize,
};
use serde_json::Value;
use url::Url;
use regex::Regex;


#[derive(Debug, Serialize, Deserialize)]
pub enum Fetched {
    NotFound,
    NoText,
    WithError {
        json: Value,
        error: String,
    },
    Records(Vec<Record>)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: u64,
    #[serde(rename = "Коробка передач")]
    pub transmission: Option<String>,

    #[serde(rename = "Объем двигателя, л")]
    pub engine_displacement: Option<f64>,
    #[serde(rename = "Рабочий объем, см³")]
    pub engine_displacement_precise: Option<u16>,

    #[serde(rename = "Привод")]
    pub drive: Option<String>,
    #[serde(rename = "Тип двигателя")]
    pub fuel_type: Option<String>,

    #[serde(rename = "Мощность, л.с.")]
    pub engine_power: Option<u16>,
    #[serde(rename = "Максимальная скорость, км/ч")]
    pub maximum_speed: Option<u16>,
    #[serde(rename = "Разгон до 100 км/ч, с")]
    pub acceleration: Option<f64>,

    #[serde(rename = "Страна происхождения бренда")]
    pub brand_country: Option<String>,
    #[serde(rename = "Страна сборки")]
    pub assembly_country: Option<String>,

    #[serde(rename = "Количество мест")]
    pub number_of_seats: Option<u8>,

    #[serde(rename = "Рейтинг EuroNCAP")]
    pub rating: Option<String>,

    #[serde(rename = "Количество цилиндров")]
    pub number_of_cylinders: Option<u8>,
    #[serde(rename = "Конфигурация")]
    pub configuration: Option<String>,

    #[serde(rename = "Крутящий момент, Н⋅м")]
    pub torque: Option<u16>,
    #[serde(rename = "Обороты максимального крутящего момента, об/мин")]
    pub torque_max: Option<String>,
    #[serde(rename = "Обороты максимальной мощности, об/мин")]
    pub max_power_speed: Option<String>,

    #[serde(rename = "Высота, мм")]
    pub height: Option<u16>,
    #[serde(rename = "Длина, мм")]
    pub length: Option<u16>,
    #[serde(rename = "Диаметр разворота, м")]
    pub turning_diameter: Option<f64>,
    #[serde(rename = "Дорожный просвет, мм")]
    pub clearance: Option<u16>,
    #[serde(rename = "Колесная база, мм")]
    pub wheelbase: Option<u16>,
    #[serde(rename = "Колея задняя, мм")]
    pub rear_track: Option<u16>,
    #[serde(rename = "Колея передняя, мм")]
    pub front_track: Option<u16>,

    #[serde(rename = "Объем багажника, л")]
    pub trunk_volume: Option<String>,

    #[serde(rename = "Емкость топливного бака, л")]
    pub fuel_tank_capacity: Option<u16>,

    #[serde(rename = "Расход топлива в городе, л/100 км")]
    pub fuel_consumption_city: Option<f64>,
    #[serde(rename = "Расход топлива по трассе, л/100 км")]
    pub fuel_consumption_highway: Option<f64>,
    #[serde(rename = "Расход топлива смешанный, л/100 км")]
    pub fuel_consumption_mixed: Option<f64>,

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

    #[serde(rename = "Задняя подвеска")]
    pub rear_suspension: Option<String>,
    #[serde(rename = "Передняя подвеска")]
    pub front_suspension: Option<String>,
}

impl Record {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            transmission: None,
            engine_power: None,
            engine_displacement: None,
            drive: None,
            acceleration: None,
            fuel_consumption_mixed: None,
            fuel_type: None,
            brand_country: None,
            number_of_seats: None,
            rating: None,
            assembly_country: None,
            number_of_cylinders: None,
            configuration: None,
            torque: None,
            torque_max: None,
            max_power_speed: None,
            engine_displacement_precise: None,
            height: None,
            turning_diameter: None,
            length: None,
            clearance: None,
            wheelbase: None,
            rear_track: None,
            front_track: None,
            trunk_volume: None,
            fuel_tank_capacity: None,
            maximum_speed: None,
            fuel_consumption_city: None,
            fuel_consumption_highway: None,
            environmental_class: None,
            rear_breaks: None,
            front_breaks: None,
            rear_tire_dimension: None,
            front_tire_dimension: None,
            rear_suspension: None,
            front_suspension: None,
        }
    }
}

use std::fs::File;
use std::io::prelude::*;
// type Item = str;
impl Fetched {
    pub fn parse<S: AsRef<str>>(text: S, url: Url) -> Result<Self> {
        lazy_static! {
            static ref RE_MATCH: Regex = Regex::new(r#"data-autocatalog="([^"]+)""#).unwrap();
            static ref RE_REPLACE: Regex = Regex::new(r#"&quot;"#).unwrap();
            static ref RE_REMOVE: Regex = Regex::new(r"\ufeff").unwrap();
        }
        match RE_MATCH.captures(text.as_ref()) {
            None => { 
                let msg = format!("data-autocatalog= NOT FOUND at {}: {}", url, text.as_ref());
                let mut file = File::create("error.txt")?;
                file.write_all(msg.as_bytes())?;
                bail!("{}", msg);
            },
            Some(caps) => {
                let data_autocatalog = caps.get(1).unwrap().as_str();
                let decoded = RE_REPLACE.replace_all(data_autocatalog, r#"""#);
                let decoded = RE_REMOVE.replace_all(&decoded, "");
                let json: Json = Json::from_str(&decoded, url)?;
                let vec_record = Self::parse_json(&json)?;
                Ok(Fetched::Records(vec_record))
            },
        }
    }
    fn parse_json(json: &Json) -> Result<Vec<Record>> {
        let mut ret: Vec<Record> = Vec::new();
        let items = json.get([By::key("modifications"), By::key("items")])?;
        for item in items.iter_vec()? {
            let id = item.get([By::key("id")])?.as_u64()?;
            let mut record = Record::new(id);
            let specification_blocks = item.get([By::key("specification"), By::key("blocks")])?;
            for block in specification_blocks.iter_vec()? {
                let params = block.get([By::key("params")])?;
                for param in params.iter_vec()? {
                    let name = param.get([By::key("name")])?;
                    let value = param.get([By::key("value")])?;
                    if value.value.is_string() && value.as_str()? == "-" {
                        continue
                    }
                    Self::update_record(&mut record, &name, &value)
                        .context(format!("while parsing {:?}", name.as_str()))?
                    ;
                }
            }
            ret.push(record);
        }
        Ok(ret)
    }
    fn update_record(record: &mut Record, name: &Json, value: &Json) -> Result<()> {
        match name.as_str()? {
            "Коробка передач" => {
                record.transmission = Some(value.as_string()?);
            },
            "Мощность, л.с." => {
                record.engine_power = Some(value.parse_as_u16()?);
            },
            "Объем двигателя, л" => {
                record.engine_displacement = Some(value.parse_as_f64()?);
            },
            "Привод" => {
                record.drive = Some(value.as_str()?.to_owned());
            },
            "Разгон до 100 км/ч, с" => {
                record.acceleration = Some(value.parse_as_f64()?);
            },
            "Тип двигателя" => {
                record.fuel_type = Some(value.as_string()?);
            },
            "Страна происхождения бренда" => {
                record.brand_country = Some(value.as_string()?);
            },
            "Количество мест" => {
                record.number_of_seats = Some(value.parse_as_u8()?);
            },
            "Рейтинг EuroNCAP" => {
                record.rating = Some(value.as_string()?);
            },
            "Страна сборки" => {
                record.assembly_country = Some(value.as_string()?);
            },
            "Количество цилиндров" => {
                record.number_of_cylinders = Some(value.parse_as_u8()?);
            },
            "Конфигурация" => {
                record.configuration = Some(value.as_string()?);
            },
            "Крутящий момент, Н⋅м" => {
                record.torque = Some(value.parse_as_u16()?);
            },
            "Обороты максимального крутящего момента, об/мин" => {
                record.torque_max = Some(value.as_string()?);
            },
            "Обороты максимальной мощности, об/мин" => {
                record.max_power_speed = Some(value.as_string()?);
            },
            "Рабочий объем, см³" => {
                record.engine_displacement_precise = Some(value.parse_as_u16()?);
            },
            "Высота, мм" => {
                record.height = Some(value.parse_as_u16()?);
            },
            "Диаметр разворота, м" => {
                record.turning_diameter = Some(value.parse_as_f64()?);
            },
            "Длина, мм" => {
                record.length = Some(value.parse_as_u16()?);
            },
            "Дорожный просвет, мм" => {
                record.clearance = Some(value.parse_as_u16()?);
            },
            "Колесная база, мм" => {
                record.wheelbase = Some(value.parse_as_u16()?);
            },
            "Колея задняя, мм" => {
                record.rear_track = Some(value.parse_as_u16()?);
            },
            "Колея передняя, мм" => {
                record.front_track = Some(value.parse_as_u16()?);
            },
            "Объем багажника, л" => {
                record.trunk_volume = Some(value.as_string()?);
            },
            "Емкость топливного бака, л" => {
                record.fuel_tank_capacity = Some(value.parse_as_u16()?);
            },
            "Максимальная скорость, км/ч" => {
                record.maximum_speed = Some(value.parse_as_u16()?);
            },
            "Расход топлива в городе, л/100 км" => {
                record.fuel_consumption_city = Some(value.parse_as_f64()?);
            },
            "Расход топлива по трассе, л/100 км" => {
                record.fuel_consumption_highway = Some(value.parse_as_f64()?);
            },
            "Расход топлива смешанный, л/100 км" => {
                record.fuel_consumption_mixed = Some(value.parse_as_f64()?);
            },
            "Экологический класс" => {
                record.environmental_class = Some(value.as_string()?);
            },
            "Задние тормоза" => {
                record.rear_breaks = Some(value.as_string()?);
            },
            "Передние тормоза" => {
                record.front_breaks = Some(value.as_string()?);
            },
            "Размерность задних шин" => {
                record.rear_tire_dimension = Some(value.as_string()?);
            },
            "Размерность передних шин" => {
                record.front_tire_dimension = Some(value.as_string()?);
            },
            "Задняя подвеска" => {
                record.rear_suspension = Some(value.as_string()?);
            },
            "Передняя подвеска" => {
                record.front_suspension = Some(value.as_string()?);
            },
            s @ _ => {
                warn!("name: {}, value: {}", s, value.value);
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| pretty_env_logger::init());
    }

    use tokio::fs::File;
    use tokio::prelude::*;

    use std::path::Path;
    #[tokio::test]
    async fn test_parse_json() -> Result<()> {
        init();

        let file_path = Path::new("test_data/autocatalog.json");
        let json = Json::from_file(file_path).await?;
        let _item = "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896";
        let _fetched = Fetched::parse_json(&json)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_only() -> Result<()> {
        init();

        let mut file = File::open("test_data/page.html").await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let text = std::str::from_utf8(&contents)?;
        let url = Url::parse("https://avito.ru/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896")?;
        let fetched = Fetched::parse(text, url)?;

        match fetched {
            Fetched::Records(vec_record) => {
                assert_eq!(vec_record.len(), 31);
            },
            _ => unreachable!(),
        }

        Ok(())
    }
}
