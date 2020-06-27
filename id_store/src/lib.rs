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
pub struct IdStore (HashMap<String, IdStoreItem>);

impl IdStore {
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
    pub fn set_ids(&mut self, key: &str, ret: ids::Ret) {
        // let key = format!("{}", params);
        let val = IdStoreItem {
            timestamp: chrono::Utc::now(),
            ret,
        };
        self.0.insert(key.to_owned(), val);
    }
    pub fn get_ids(&self, key: &str, fresh_duration: chrono::Duration) -> Option<&IdStoreItem> {
        // let key = format!("{}", arg);
        match self.0.get(key) {
            None => None,
            Some(item) => 
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

impl FromStr for IdStore {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = serde_json::from_str(s)?;
        Ok(ret)
    }
}

// ============================================================================

pub struct IdStoreItem {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ret: ids::Ret,
}

impl Serialize for IdStoreItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 2 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("IdStoreItem", 2)?;
        let timestamp = self.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        state.serialize_field("timestamp", &timestamp)?;
        let mut sorted = self.ret.iter().collect::<Vec<&u64>>();
        sorted.sort_unstable();
        state.serialize_field("ret", &sorted)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for IdStoreItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field { Timestamp, Ret }

        struct IdStoreItemVisitor;

        impl<'de> Visitor<'de> for IdStoreItemVisitor {
            type Value = IdStoreItem;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct IdStoreItem")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<IdStoreItem, V::Error>
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
                Ok(IdStoreItem{timestamp, ret})
            }

            fn visit_map<V>(self, mut map: V) -> Result<IdStoreItem, V::Error>
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
                Ok(IdStoreItem{timestamp, ret})
            }
        }

        const FIELDS: &'static [&'static str] = &["timestamp", "ret"];
        deserializer.deserialize_struct("IdStoreItem", FIELDS, IdStoreItemVisitor)
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
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    use std::collections::HashSet;

    #[tokio::test]
    async fn to_file() -> Result<()> {
        init();

        let key = "categoryId=9&locationId=637640&searchRadius=0&privateOnly=1&sort=date&owner[]=private";
        let mut ids: ids::Ret = HashSet::new();
        ids.insert(1);
        ids.insert(2);
        ids.insert(3);
        ids.insert(5);
        ids.insert(8);
        ids.insert(13);

        let mut diap_store = IdStore::new();
        diap_store.set_ids(key, ids);

        let json = serde_json::to_string_pretty(&diap_store)?;
        let file_path = Path::new("out_test/ids.json");
        diap_store.to_file(&file_path).await?;
        let diap_restore = IdStore::from_file(&file_path).await?;
        let json_restore = serde_json::to_string_pretty(&diap_restore)?;
        pretty_assertions::assert_eq!(json_restore, json);

        Ok(())
    }

    #[tokio::test]
    async fn from_file() -> Result<()> {
        init();

        let err = IdStore::from_file(Path::new("out_test/ids_corrupted.json")).await;
        assert!(err.is_err());
        let err = err.err().unwrap();
        assert_eq!(
            "invalid value: string \"some2020-06-27T16:14:50Z\", expected rfc_3339 timestamp at line 3 column 43", 
            &format!("{}", err),
        );

        Ok(())
    }
}

