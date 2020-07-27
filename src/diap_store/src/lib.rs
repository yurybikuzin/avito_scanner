#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

use std::fmt;

use serde::{
    Serialize, 
    Deserialize,
    ser::{Serializer, SerializeStruct},
    de::{self, Deserializer, Visitor, SeqAccess, MapAccess, Unexpected},
};
use std::collections::HashMap;

use std::path::Path;
use std::str::FromStr;

use tokio::fs::{self, File};
use tokio::prelude::*;

// ============================================================================
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct DiapStore (HashMap<String, DiapStoreItem>);

impl DiapStore {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub async fn to_file(&self, file_path: &Path) -> Result<()> {
        if let Some(dir_path) = file_path.parent() {
            fs::create_dir_all(dir_path).await?;
        }
        let mut file = File::create(file_path).await?;
        let json = serde_json::to_string_pretty(&self)?;
        file.write_all(json.as_bytes()).await?;
        Ok(())
    }
    pub async fn from_file(file_path: &Path) -> Result<Self> {
        let mut file = File::open(file_path).await?;
        let mut content = vec![];
        file.read_to_end(&mut content).await?;
        let content = std::str::from_utf8(&content)?;
        let ret = Self::from_str(content)?;
        Ok(ret)
    }
    pub fn set_diaps(&mut self, arg: &diaps::Arg, ret: diaps::Ret) {
        let key = format!("{}", arg);
        let val = DiapStoreItem {
            timestamp: chrono::Utc::now(),
            ret,
        };
        self.0.insert(key, val);
    }
    pub fn get_diaps(&self, arg: &diaps::Arg, fresh_duration: chrono::Duration) -> Option<&DiapStoreItem> {
        let key = format!("{}", arg);
        info!("diaps key: {}", key);
        match self.0.get(&key) {
            None => {
                info!("no diaps for key: {}, diaps: {}", key, serde_json::to_string_pretty(&self).unwrap());
                None
            },
            Some(item) => {
                info!("got diaps by key: {}", key);
                if let Some(fresh_limit) = item.timestamp.checked_add_signed(fresh_duration) {
                    if chrono::Utc::now() > fresh_limit {
                        None
                    } else {
                        Some(item)
                    }
                } else {
                    None
                }
            }
        }
    }
}

impl FromStr for DiapStore {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = serde_json::from_str(s)?;
        Ok(ret)
    }
}

// ============================================================================

pub struct DiapStoreItem {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ret: diaps::Ret,
}

impl Serialize for DiapStoreItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 2 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("DiapStoreItem", 2)?;
        let timestamp = self.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        state.serialize_field("timestamp", &timestamp)?;
        state.serialize_field("ret", &self.ret)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for DiapStoreItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field { Timestamp, Ret }

        struct DiapStoreItemVisitor;

        impl<'de> Visitor<'de> for DiapStoreItemVisitor {
            type Value = DiapStoreItem;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct DiapStoreItem")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<DiapStoreItem, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let val: String = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let val = chrono::DateTime::parse_from_rfc3339(&val)
                    .map_err(|_err| 
                        de::Error::invalid_value(Unexpected::Str(&val), &"rfc_3339 timestamp")
                    )?;
                let val = val.with_timezone(&chrono::Utc);
                let timestamp = val;

                let ret = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(DiapStoreItem{timestamp, ret})
            }

            fn visit_map<V>(self, mut map: V) -> Result<DiapStoreItem, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut timestamp = None;
                let mut ret = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Timestamp => {
                            if timestamp.is_some() {
                                return Err(de::Error::duplicate_field("timestamp"));
                            }
                            let val: String = map.next_value()?;
                            let val = chrono::DateTime::parse_from_rfc3339(&val)
                                .map_err(|_err| 
                                    de::Error::invalid_value(Unexpected::Str(&val), &"rfc_3339 timestamp")
                                )?;
                            let val = val.with_timezone(&chrono::Utc);
                            timestamp = Some(val);
                        }
                        Field::Ret => {
                            if ret.is_some() {
                                return Err(de::Error::duplicate_field("ret"));
                            }
                            ret = Some(map.next_value()?);
                        }
                    }
                }
                let timestamp = timestamp.ok_or_else(|| de::Error::missing_field("timestamp"))?;
                let ret = ret.ok_or_else(|| de::Error::missing_field("ret"))?;
                Ok(DiapStoreItem{timestamp, ret})
            }
        }

        const FIELDS: &'static [&'static str] = &["timestamp", "ret"];
        deserializer.deserialize_struct("DiapStoreItem", FIELDS, DiapStoreItemVisitor)
    }
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;

    #[tokio::test]
    async fn test_to_file() -> Result<()> {
        test_helper::init();

        let pool = rmq::get_pool();
        let client_provider = client::Provider::new(client::Kind::ViaProxy(pool));
        let arg = diaps::Arg {
            params: "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private",
            count_limit: 4900,
            price_precision: 20000,
            price_max_inc: 1000000,
            client_provider,
        };
        let mut diaps: Vec<diaps::Diap> = Vec::new();
        diaps.push(diaps::Diap { price_min: None, price_max: Some(234376), count: 4761, checks: 7});
        diaps.push(diaps::Diap { price_min: Some(234377), price_max: Some(379514), count: 4864, checks: 9});
        diaps.push(diaps::Diap { price_min: Some(520616), price_max: Some(739205), count: 4708, checks: 8 });
        diaps.push(diaps::Diap { price_min: Some(739206), price_max: Some(1200685), count: 4829, checks: 12 });
        diaps.push(diaps::Diap { price_min: Some(1200685), price_max: Some(2200686), count: 3355, checks: 2 });
        diaps.push(diaps::Diap { price_min: Some(2200687), price_max: None, count: 1735, checks: 1 });
        let ret = diaps::Ret {
            diaps,
            checks_total: 48,
            last_stamp: 0,
        };
        let mut diap_store = DiapStore::new();
        diap_store.set_diaps(&arg, ret);

        let json = serde_json::to_string_pretty(&diap_store)?;
        let file_path = Path::new("out_test/diaps.json");
        diap_store.to_file(&file_path).await?;
        let diap_restore = DiapStore::from_file(&file_path).await?;
        let json_restore = serde_json::to_string_pretty(&diap_restore)?;
        pretty_assertions::assert_eq!(json_restore, json);

        Ok(())
    }

    #[tokio::test]
    async fn test_from_file() -> Result<()> {
        test_helper::init();

        let err = DiapStore::from_file(Path::new("out_test/diaps_corrupted.json")).await;
        assert!(err.is_err());
        let err = err.err().unwrap();
        assert_eq!(
            "invalid value: string \"some2020-06-25T21:20:53Z\", expected rfc_3339 timestamp at line 3 column 43", 
            &format!("{}", err),
        );

        Ok(())
    }
}

