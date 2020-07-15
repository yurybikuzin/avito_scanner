#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use serde::ser::{Serialize, Serializer, SerializeStruct};
use std::fmt;
use serde::de::{self, 
    Deserializer, Visitor, 
    MapAccess};

use std::time::Duration;
#[derive(Debug)]
pub struct Req {
    pub correlation_id: uuid::Uuid,
    pub reply_to: String,
    pub method: reqwest::Method,
    pub url: reqwest::Url,
    pub timeout: Option<Duration>,
    // pub no_proxy: Option<bool>,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Field { 
    CorrelationId,
    ReplyTo,
    Method, 
    Url, 
    Timeout,
    // NoProxy,
}

const FIELDS: &'static [&'static str] = &["correlation_id", "reply_to", "method", "url", "timeout"
// , "no_proxy"
];

use serde::{Deserialize};
impl Serialize for Req {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Req", FIELDS.len())?;
        for field in FIELDS {
            match *field {
                "correlation_id" => {
                    state.serialize_field("correlation_id", &self.correlation_id.to_string())?;
                },
                "reply_to" => {
                    state.serialize_field("reply_to", &self.reply_to.as_str())?;
                },
                "method" => {
                    let method = match self.method {
                        reqwest::Method::GET => "GET",
                        reqwest::Method::POST => "POST",
                        reqwest::Method::PUT => "PUT",
                        reqwest::Method::DELETE => "DELETE",
                        reqwest::Method::HEAD => "HEAD",
                        reqwest::Method::OPTIONS => "OPTIONS",
                        reqwest::Method::CONNECT => "CONNECT",
                        reqwest::Method::PATCH => "PATCH",
                        reqwest::Method::TRACE => "TRACE",
                        _ => unreachable!(),
                    };
                    state.serialize_field("method", &method)?;
                },
                "url" => {
                    state.serialize_field("url", &self.url.as_str())?;
                },
                "timeout" => {
                    if let Some(timeout) = &self.timeout {
                        state.serialize_field("timeout", &timeout.as_secs())?;
                    }
                },
                // "no_proxy" => {
                //     if let Some(no_proxy) = &self.no_proxy {
                //         state.serialize_field("no_proxy", &no_proxy)?;
                //     }
                // },
                _ => {
                    return Err(serde::ser::Error::custom(format!("unreachable for unknown field {}", field)));
                },
            }
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for Req {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct("Req", FIELDS, ReqVisitor)
    }
}

struct ReqVisitor;

impl<'de> Visitor<'de> for ReqVisitor {
    type Value = Req;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Req")
    }

    fn visit_map<V>(self, mut map: V) -> std::result::Result<Req, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut correlation_id = None;
        let mut reply_to = None;
        let mut method = None;
        let mut url = None;
        let mut timeout = None;
        while let Some(key) = map.next_key()? {
            match key {
                Field::CorrelationId => {
                    if correlation_id.is_some() {
                        return Err(de::Error::duplicate_field("correlation_id"));
                    }
                    let s: String = map.next_value()?;
                    correlation_id = match uuid::Uuid::parse_str(&s) {
                        Ok(uuid) => Some(uuid),
                        Err(_) => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s.as_ref()), &"valid UUID")),
                    };
                }
                Field::ReplyTo => {
                    if reply_to.is_some() {
                        return Err(de::Error::duplicate_field("reply_to"));
                    }
                    reply_to = Some(map.next_value()?);
                }
                Field::Method => {
                    if method.is_some() {
                        return Err(de::Error::duplicate_field("method"));
                    }
                    let s: String = map.next_value()?;
                    method = Some(match s.as_ref() {
                        "GET" => reqwest::Method::GET,
                        "POST" => reqwest::Method::POST,
                        "PUT" => reqwest::Method::PUT,
                        "DELETE" => reqwest::Method::DELETE,
                        "HEAD" => reqwest::Method::HEAD,
                        "OPTIONS" => reqwest::Method::OPTIONS,
                        "CONNECT" => reqwest::Method::CONNECT,
                        "PATCH" => reqwest::Method::PATCH,
                        "TRACE" => reqwest::Method::TRACE,
                        _ => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s.as_ref()), &"valid HTTP method")),
                    });
                }
                Field::Url => {
                    if url.is_some() {
                        return Err(de::Error::duplicate_field("url"));
                    }
                    let s: String = map.next_value()?;
                    url = match reqwest::Url::parse(s.as_ref()) {
                        Err(_) => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s.as_ref()), &"valid url")),
                        Ok(url) => Some(url),
                    };
                }
                Field::Timeout => {
                    if timeout.is_some() {
                        return Err(de::Error::duplicate_field("timeout"));
                    }
                    timeout = Some( Duration::from_secs(map.next_value()? ));
                }
                // Field::NoProxy => {
                //     if no_proxy.is_some() {
                //         return Err(de::Error::duplicate_field("no_proxy"));
                //     }
                //     no_proxy = Some(map.next_value()?);
                // }
            }
        }
        let correlation_id = correlation_id.ok_or_else(|| de::Error::missing_field("correlation_id"))?;
        let reply_to = reply_to.ok_or_else(|| de::Error::missing_field("reply_to"))?;
        let method = method.ok_or_else(|| de::Error::missing_field("method"))?;
        let url = url.ok_or_else(|| de::Error::missing_field("url"))?;
        Ok(Req {correlation_id, reply_to, method, url, timeout})
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
    async fn test_req() -> Result<()> {
        init();

        let json = r#"{
            "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
            "reply_to": "response",
            "method": "GET",
            "url": "https://bikuzin.baza-winner.ru/echo",
            "timeout": 5
        }"#;
        let req: Req = serde_json::from_str(json).unwrap();
        let tst = serde_json::to_string(&req).unwrap();
        let eta = r#"{"correlation_id":"550e8400-e29b-41d4-a716-446655440000","reply_to":"response","method":"GET","url":"https://bikuzin.baza-winner.ru/echo","timeout":5}"#;
        assert_eq!(tst, eta);

        let json = r#"{
            "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
            "reply_to": "response",
            "method": "GET",
            "url": "https://bikuzin.baza-winner.ru/echo",
            "timeout": 5
        }"#;
        let req: Req = serde_json::from_str(json).unwrap();
        let tst = serde_json::to_string(&req).unwrap();
        let eta = r#"{"correlation_id":"550e8400-e29b-41d4-a716-446655440000","reply_to":"response","method":"GET","url":"https://bikuzin.baza-winner.ru/echo","timeout":5}"#;
        assert_eq!(tst, eta);

        let json = r#"{
            "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
            "reply_to": "response",
            "method": "GET",
            "url": "https://bikuzin.baza-winner.ru/echo"
        }"#;
        let req: Req = serde_json::from_str(json).unwrap();
        let tst = serde_json::to_string(&req).unwrap();
        let eta = r#"{"correlation_id":"550e8400-e29b-41d4-a716-446655440000","reply_to":"response","method":"GET","url":"https://bikuzin.baza-winner.ru/echo"}"#;
        assert_eq!(tst, eta);

        let json = r#"{
            "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
            "reply_to": "response",
            "method": "GET",
            "url": "https://bikuzin.baza-winner.ru/echo",
            "proxy": false
        }"#;
        let err = serde_json::from_str::<Req>(json).unwrap_err();
        let tst = format!("{}", err);
        let eta = r#"unknown field `proxy`, expected one of `correlation_id`, `reply_to`, `method`, `url`, `timeout` at line 6 column 19"#.to_owned();
        assert_eq!(tst, eta); 

        Ok(())
    }

}

