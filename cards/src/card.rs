
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
    pub body_type: Option<String>,
    pub brand: Option<String>,
    pub color: Option<String>,
    pub fuel_type: Option<String>,
    // modelDate - год модели
    pub name: Option<String>,
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

    pub auto_catalog_url: Option<String>,

    pub item_price: Option<u64>,
    pub market_price: Option<u64>,

    pub status: Option<String>,
    pub closing_reason: Option<String>,
    pub type_of_trade: Option<String>,
    pub canonical_url: Option<String>,
    pub avito_id: u64,
}

