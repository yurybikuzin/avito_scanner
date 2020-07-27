
use serde::{
    Serialize, 
    Deserialize,
};

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

