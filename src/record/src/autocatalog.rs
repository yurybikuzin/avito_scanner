
use serde::{
    Serialize, 
    Deserialize,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: u64,
    pub title: String,

    #[serde(rename = "Коробка передач")]
    pub transmission: Option<String>,

    #[serde(rename = "Объем двигателя, л")]
    pub engine_displacement: Option<String>,
    #[serde(rename = "Рабочий объем, см³")]
    pub engine_displacement_precise: Option<String>,

    #[serde(rename = "Привод")]
    pub drive: Option<String>,
    #[serde(rename = "Тип двигателя")]
    pub fuel_type: Option<String>,

    #[serde(rename = "Мощность, л.с.")]
    pub engine_power: Option<String>,
    #[serde(rename = "Максимальная скорость, км/ч")]
    pub maximum_speed: Option<String>,
    #[serde(rename = "Разгон до 100 км/ч, с")]
    pub acceleration: Option<String>,

    #[serde(rename = "Страна происхождения бренда")]
    pub brand_country: Option<String>,
    #[serde(rename = "Страна сборки")]
    pub assembly_country: Option<String>,

    #[serde(rename = "Количество мест")]
    pub number_of_seats: Option<String>,

    #[serde(rename = "Рейтинг EuroNCAP")]
    pub rating: Option<String>,

    #[serde(rename = "Количество цилиндров")]
    pub number_of_cylinders: Option<String>,
    #[serde(rename = "Конфигурация")]
    pub configuration: Option<String>,

    #[serde(rename = "Крутящий момент, Н⋅м")]
    pub torque: Option<String>,
    #[serde(rename = "Обороты максимального крутящего момента, об/мин")]
    pub torque_max: Option<String>,
    #[serde(rename = "Обороты максимальной мощности, об/мин")]
    pub max_power_speed: Option<String>,

    #[serde(rename = "Высота, мм")]
    pub height: Option<String>,
    #[serde(rename = "Длина, мм")]
    pub length: Option<String>,
    #[serde(rename = "Диаметр разворота, м")]
    pub turning_diameter: Option<String>,
    #[serde(rename = "Дорожный просвет, мм")]
    pub clearance: Option<String>,
    #[serde(rename = "Колесная база, мм")]
    pub wheelbase: Option<String>,
    #[serde(rename = "Колея задняя, мм")]
    pub rear_track: Option<String>,
    #[serde(rename = "Колея передняя, мм")]
    pub front_track: Option<String>,

    #[serde(rename = "Объем багажника, л")]
    pub trunk_volume: Option<String>,

    #[serde(rename = "Емкость топливного бака, л")]
    pub fuel_tank_capacity: Option<String>,

    #[serde(rename = "Расход топлива в городе, л/100 км")]
    pub fuel_consumption_city: Option<String>,
    #[serde(rename = "Расход топлива по трассе, л/100 км")]
    pub fuel_consumption_highway: Option<String>,
    #[serde(rename = "Расход топлива смешанный, л/100 км")]
    pub fuel_consumption_mixed: Option<String>,

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

    #[serde(rename = "Мировая премьера")]
    pub world_premier: Option<String>,
    #[serde(rename = "Ожидаемое обновление")]
    pub pending_update: Option<String>,
    #[serde(rename = "Ширина с (зеркалами), мм")]
    pub width_with_mirrors: Option<String>,
    #[serde(rename = "Размерность задних дисков")]
    pub rear_disc_dimension: Option<String>,
    #[serde(rename = "Размерность передних дисков")]
    pub front_disc_dimension: Option<String>,
}

impl Record {
    pub fn new(id: u64, title: String) -> Self {
        Self {
            id,
            title,
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
            world_premier: None,
            front_disc_dimension: None,
            rear_disc_dimension: None,
            pending_update: None,
            width_with_mirrors: None,
        }
    }
}

