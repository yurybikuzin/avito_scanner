
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
    NoAutocatalog {
        url: String,
        text: String,
    },
    WithError {
        json: Value,
        error: String,
    },
    Records(Vec<Record>)
}

pub type Record = record::autocatalog::Record;

// use tokio::fs;
// use tokio::prelude::*;
// use std::fs::File;
// use std::io::prelude::*;
// type Item = str;
impl Fetched {
    pub async fn parse<S: AsRef<str>>(text: S, url: Url) -> Result<Self> {
        lazy_static! {
            static ref RE_MATCH: Regex = Regex::new(r#"data-autocatalog="([^"]+)""#).unwrap();
            static ref RE_REPLACE: Regex = Regex::new(r#"&quot;"#).unwrap();
            static ref RE_REMOVE: Regex = Regex::new(r"\ufeff").unwrap();
        }
        match RE_MATCH.captures(text.as_ref()) {
            None => { 
                error!("data-autocatalog= NOT FOUND at {}", url);
                Ok(Fetched::NoAutocatalog{url: url.as_ref().to_owned(), text: text.as_ref().to_owned()})
            },
            Some(caps) => {
                let data_autocatalog = caps.get(1).unwrap().as_str();
                let decoded = RE_REPLACE.replace_all(data_autocatalog, r#"""#);
                let decoded = RE_REMOVE.replace_all(&decoded, "");
                let json: Json = Json::from_str(&decoded, url)?;
                let vec_record = Self::parse_json(&json).await?;
                Ok(Fetched::Records(vec_record))
            },
        }
    }
    async fn parse_json(json: &Json) -> Result<Vec<Record>> {
        let mut ret: Vec<Record> = Vec::new();
        let items = json.get([By::key("modifications"), By::key("items")])?;
        for item in items.iter_vec()? {
            let id = item.get([By::key("id")])?.as_u64()?;
            let title = item.get([By::key("title")])?.as_string()?;
            let mut record = Record::new(id, title);

    // let mut file = fs::File::create(format!("autocatalog-{}.json", id)).await?;
    // file.write_all(serde_json::to_string_pretty(&item.value)?.as_bytes()).await?;

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
                record.engine_power = Some(value.as_string()?);
            },
            "Объем двигателя, л" => {
                record.engine_displacement = Some(value.as_string()?);
            },
            "Привод" => {
                record.drive = Some(value.as_string()?);
            },
            "Разгон до 100 км/ч, с" => {
                record.acceleration = Some(value.as_string()?);
            },
            "Тип двигателя" => {
                record.fuel_type = Some(value.as_string()?);
            },
            "Страна происхождения бренда" => {
                record.brand_country = Some(value.as_string()?);
            },
            "Количество мест" => {
                record.number_of_seats = Some(value.as_string()?);
            },
            "Рейтинг EuroNCAP" => {
                record.rating = Some(value.as_string()?);
            },
            "Страна сборки" => {
                record.assembly_country = Some(value.as_string()?);
            },
            "Количество цилиндров" => {
                record.number_of_cylinders = Some(value.as_string()?);
            },
            "Конфигурация" => {
                record.configuration = Some(value.as_string()?);
            },
            "Крутящий момент, Н⋅м" => {
                record.torque = Some(value.as_string()?);
            },
            "Обороты максимального крутящего момента, об/мин" => {
                record.torque_max = Some(value.as_string()?);
            },
            "Обороты максимальной мощности, об/мин" => {
                record.max_power_speed = Some(value.as_string()?);
            },
            "Рабочий объем, см³" => {
                record.engine_displacement_precise = Some(value.as_string()?);
            },
            "Высота, мм" => {
                record.height = Some(value.as_string()?);
            },
            "Диаметр разворота, м" => {
                record.turning_diameter = Some(value.as_string()?);
            },
            "Длина, мм" => {
                record.length = Some(value.as_string()?);
            },
            "Дорожный просвет, мм" => {
                record.clearance = Some(value.as_string()?);
            },
            "Колесная база, мм" => {
                record.wheelbase = Some(value.as_string()?);
            },
            "Колея задняя, мм" => {
                record.rear_track = Some(value.as_string()?);
            },
            "Колея передняя, мм" => {
                record.front_track = Some(value.as_string()?);
            },
            "Объем багажника, л" => {
                record.trunk_volume = Some(value.as_string()?);
            },
            "Емкость топливного бака, л" => {
                record.fuel_tank_capacity = Some(value.as_string()?);
            },
            "Максимальная скорость, км/ч" => {
                record.maximum_speed = Some(value.as_string()?);
            },
            "Расход топлива в городе, л/100 км" => {
                record.fuel_consumption_city = Some(value.as_string()?);
            },
            "Расход топлива по трассе, л/100 км" => {
                record.fuel_consumption_highway = Some(value.as_string()?);
            },
            "Расход топлива смешанный, л/100 км" => {
                record.fuel_consumption_mixed = Some(value.as_string()?);
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
            "Размерность задних дисков" => {
                record.rear_disc_dimension = Some(value.as_string()?);
            },
            "Размерность передних дисков" => {
                record.front_disc_dimension = Some(value.as_string()?);
            },
            "Ширина (с зеркалами), мм" => {
                record.width_with_mirrors = Some(value.as_string()?);
            },
            "Ожидаемое обновление" => {
                record.pending_update = Some(value.as_string()?);
            },
            "Мировая премьера" => {
                record.world_premier = Some(value.as_string()?);
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
    // use tokio::prelude::*;

    use std::path::Path;
    #[tokio::test]
    async fn test_parse_json() -> Result<()> {
        init();

        let file_path = Path::new("test_data/autocatalog.json");
        let json = Json::from_file(file_path).await?;
        let _item = "/autocatalog/bmw/5-seriya/e60e61-20022010/sedan/363896";
        let _fetched = Fetched::parse_json(&json).await?;

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
        let fetched = Fetched::parse(text, url).await?;

        match fetched {
            Fetched::Records(vec_record) => {
                assert_eq!(vec_record.len(), 31);
            },
            _ => unreachable!(),
        }

        Ok(())
    }
}
