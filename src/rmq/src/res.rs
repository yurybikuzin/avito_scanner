
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use serde::ser::{Serialize, Serializer, SerializeStruct};
use std::fmt;
use serde::de::{self, 
    Deserializer, Visitor, 
    MapAccess};

// use std::time::Duration;
#[derive(Debug)]
pub struct Res {
    pub status: http::StatusCode,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field { 
    Status,
    Text, 
}

const FIELDS: &'static [&'static str] = &["status", "text"];

use serde::{Deserialize};
impl Serialize for Res {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Res", FIELDS.len())?;
        for field in FIELDS {
            match *field {
                "status" => {
                    state.serialize_field("status", &self.status.as_u16())?;
                },
                "text" => {
                    state.serialize_field("text", &self.text.as_str())?;
                },
                _ => {
                    return Err(serde::ser::Error::custom(format!("unreachable for unknown field {}", field)));
                },
            }
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for Res {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct("Res", FIELDS, ResVisitor)
    }
}

struct ResVisitor;

impl<'de> Visitor<'de> for ResVisitor {
    type Value = Res;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Res")
    }

    fn visit_map<V>(self, mut map: V) -> std::result::Result<Res, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut status = None;
        let mut text = None;
        while let Some(key) = map.next_key()? {
            match key {
                Field::Status => {
                    if status.is_some() {
                        return Err(de::Error::duplicate_field("status"));
                    }
                    let code: u16 = map.next_value()?; 
                    status = match http::StatusCode::from_u16(code) {
                        Err(_) => return Err(de::Error::invalid_value(serde::de::Unexpected::Unsigned(code as u64), &"valid http status code: 100..599")),
                        Ok(status) => Some(status),
                    };
                }
                Field::Text => {
                    if text.is_some() {
                        return Err(de::Error::duplicate_field("text"));
                    }
                    text = Some(map.next_value()?);
                }
            }
        }
        let status = status.ok_or_else(|| de::Error::missing_field("status"))?;
        let text = text.ok_or_else(|| de::Error::missing_field("text"))?;
        Ok(Res {status, text})
    }
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};
    // #[allow(unused_imports)]
    // use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| pretty_env_logger::init());
    }


    #[tokio::test]
    async fn test_res() -> Result<()> {
        init();

        let json = r#"{
            "status": 200,
            "text": "something"
        }"#;
        let req: Res = serde_json::from_str(json).unwrap();
        let tst = serde_json::to_string(&req).unwrap();
        let eta = r#"{"status":200,"text":"something"}"#;
        assert_eq!(tst, eta);

        Ok(())
    }

}

