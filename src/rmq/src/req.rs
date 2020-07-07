
use serde::ser::{Serialize, Serializer, SerializeStruct};
use std::fmt;
use serde::de::{self, 
    Deserializer, Visitor, 
    MapAccess};

pub struct Req {
    pub method: reqwest::Method,
    pub url: reqwest::Url,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum Field { 
    Method, 
    Url, 
}

use serde::{Deserialize};
impl Serialize for Req {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // 2 is the number of fields in the struct.
        let mut state = serializer.serialize_struct("Req", 2)?;
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
        state.serialize_field("url", &self.url.as_str())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Req {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {

        const FIELDS: &'static [&'static str] = &["secs", "nanos"];
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
        let mut method = None;
        let mut url = None;
        while let Some(key) = map.next_key()? {
            match key {
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
                    url = Some(match reqwest::Url::parse(s.as_ref()) {
                        Err(_) => return Err(de::Error::invalid_value(serde::de::Unexpected::Str(s.as_ref()), &"valid url")),
                        Ok(url) => url,
                    });
                }
            }
        }
        let method = method.ok_or_else(|| de::Error::missing_field("method"))?;
        let url = url.ok_or_else(|| de::Error::missing_field("url"))?;
        Ok(Req {method, url})
    }
}
