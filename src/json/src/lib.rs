#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::fmt;

// ============================================================================
// ============================================================================

#[derive(Debug, Clone)]
pub enum By {
    Index(usize),
    Key(String),
}

impl By {
    pub fn key<S: AsRef<str>>(key: S) -> Self {
        Self::Key(key.as_ref().to_owned())
    }
    pub fn index(index: usize) -> Self {
        Self::Index(index)
    }
}

// impl AsRef<&By> for By {
//     #[inline]
//     fn as_ref(&self) -> &By {
//         &self
//     }
// }

impl fmt::Display for By {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            By::Key(key) => {
                write!(f, ".{:?}", key)?;
            },
            By::Index(index) => {
                write!(f, "[{}]", index)?;
            },
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum JsonSource {
    FilePath(std::path::PathBuf),
    Name(String),
}

impl fmt::Display for JsonSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonSource::FilePath(file_path) => {
                write!(f, "{:?}", file_path)?;
            },
            JsonSource::Name(name) => {
                write!(f, "{:?}", name)?;
            },
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct JsonPath {
    pub src: JsonSource,
    pub items: Vec<By>,
}

impl JsonPath {
    pub fn new<>(src: JsonSource) -> Self {
        Self {
            src,
            items: Vec::new(),
        }
    }
    pub fn add(&self, path_item: By) -> Self {
        let mut items = self.items.clone();
        items.push(path_item);
        let src = self.src.to_owned();
        Self {src, items}
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.src)?;
        for item in self.items.iter() {
            write!(f, "{}", item)?;
        }
        Ok(())
    }
}

use serde_json::{Value, Map, Number};
#[derive(Clone, Debug)]
pub struct Json {
    pub value: Value,
    pub path: JsonPath,
}

use tokio::fs::File;
use tokio::prelude::*;

// macro_rules! as_64 {
//     ($type: ty) => {
//         paste::item!{
//             pub fn [< as_ $type >] (&self) -> Result<u64> {
//                 match &self.value {
//                     Value::Number(n) => {
//                         if n.[< is_ $type >] () {
//                             let n = n.[< as_ $type >]().unwrap();
//                             Ok(n)
//                         } else {
//                             bail!("{} expected to be a {}, but: {}", self.path, stringify!($type), n);
//                         }
//                     },
//                     _ => {
//                         bail!("{} expected to be a {}, but: {}", self.path, stringify!($type), serde_json::to_string_pretty(&self.value)?);
//                     },
//                 }
//             }
//         }
//     }
// }
//
// macro_rules! as_u {
//     ($type: ty) => {
//         paste::item!{
//             pub fn [< as_ $type >] (&self) -> Result<$type> {
//                 let n = self.as_u64()?;
//                 let n = $type::try_from(n)
//                     .map_err(|err| Error::new(err))
//                     .context(format!("{} expected to be a {}, but: {}", self.path, stringify!($type), n))?;
//                 Ok(n)
//             }
//         }
//     }
// }

macro_rules! as_ {
    ($type: ty) => {
        paste::item!{
            pub fn [< as_ $type >] (&self) -> Result<$type> {
                match &self.value {
                    Value::Number(n) => {
                        if n.[< is_ $type >] () {
                            let n = n.[< as_ $type >]().unwrap();
                            Ok(n)
                        } else {
                            bail!("{} expected to be a {}, but: {}", self.path, stringify!($type), n);
                        }
                    },
                    _ => {
                        bail!("{} expected to be a {}, but: {}", self.path, stringify!($type), serde_json::to_string_pretty(&self.value)?);
                    },
                }
            }
        }
    };
    ($from: ty => $type: ty) => {
        paste::item!{
            pub fn [< as_ $type >] (&self) -> Result<$type> {
                let n = self.[< as_ $from >]()?;
                let n = $type::try_from(n)
                    .map_err(|err| Error::new(err))
                    .context(format!("{} expected to be a {}, but: {}", self.path, stringify!($type), n))?;
                Ok(n)
            }
        }
    }
}


impl Json {
    pub fn new(value: Value, src: JsonSource) -> Self {
        Self {
            value,
            path: JsonPath::new(src),
        }
    }
    pub async fn from_file<P: AsRef<std::path::Path>>(file_path: P) -> Result<Self> {
        let file_path = file_path.as_ref();
        let mut file = File::open(file_path).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;
        let decoded = std::str::from_utf8(&contents)?;

        let value: Value = serde_json::from_str(&decoded)?;
        Ok(Self {
            value,
            path: JsonPath::new(JsonSource::FilePath(file_path.to_owned())),
        })
    }
    pub fn get<P: AsRef<[By]>>(&self, path_items: P) -> Result<Self> {
        let mut ret = self.clone();
        for path_item in path_items.as_ref().iter() {
            ret = ret.get_by_path_item(&path_item)?;
        }
        Ok(ret)
    }
    fn get_by_path_item(&self, path_item: &By) -> Result<Self> {
        let get_ret = 
            match path_item {
                By::Key(key) => {
                    self.value.get(key)
                },
                By::Index(index) => {
                    self.value.get(index)
                },
            }
        ;
        let value: serde_json::Value = match get_ret {
            Some(value) => value.clone(),
            None => {
                bail!("{}: not found {} at {}", self.path, path_item, serde_json::to_string_pretty(&self.value)?);
            },
        };
        let path = self.path.add(path_item.to_owned());
        Ok(Self { value, path })
    }
    pub fn as_str(&self) -> Result<&str> {
        match &self.value {
            Value::String(s) => Ok(s),
            _ => {
                bail!("{} expected to be a String, but: {}", self.path, serde_json::to_string_pretty(&self.value)?);
            },
        }
    }
    pub fn as_vec(&self) -> Result<&Vec<Value>> {
        match &self.value {
            Value::Array(vec) => Ok(vec),
            _ => {
                bail!("{} expected to be an Array, but: {}", self.path, serde_json::to_string_pretty(&self.value)?);
            },
        }
    }
    pub fn as_map(&self) -> Result<&Map<String, Value>> {
        match &self.value {
            Value::Object(map) => Ok(map),
            _ => {
                bail!("{} expected to be an Object, but: {}", self.path, serde_json::to_string_pretty(&self.value)?);
            },
        }
    }
    pub fn as_null(&self) -> Result<()> {
        match &self.value {
            Value::Null => Ok(()),
            _ => {
                bail!("{} expected to be a Null, but: {}", self.path, serde_json::to_string_pretty(&self.value)?);
            },
        }
    }
    pub fn as_bool(&self) -> Result<bool> {
        match &self.value {
            Value::Bool(b) => Ok(*b),
            _ => {
                bail!("{} expected to be a Bool, but: {}", self.path, serde_json::to_string_pretty(&self.value)?);
            },
        }
    }
    as_!(u64);
    as_!(u64 => usize);
    as_!(u64 => u32);
    as_!(u64 => u16);
    as_!(u64 => u8);
    as_!(i64);
    as_!(i64 => isize);
    as_!(i64 => i32);
    as_!(i64 => i16);
    as_!(i64 => i8);
    as_!(f64);
}

use std::convert::TryFrom;

impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.path)?;
        write!(f, "{}", serde_json::to_string_pretty(&self.value).unwrap())?;
        Ok(())
    }
}


// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    // #[allow(unused_imports)]
    // use pretty_assertions::{assert_eq, assert_ne};
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    #[tokio::test]
    async fn test_json() -> Result<()> {
        init();

        let file_path = std::path::Path::new("test_data/sample.json");
        let json: Json = Json::from_file(&file_path).await?;

        let key = "defaultModificationId";
        let by: Vec<By> = vec![By::key(key)];
        let value = json.get(by)?;
        let tst = value.as_u64()?;
        assert_eq!(tst, 363896u64);

        let tst = value.as_usize()?;
        assert_eq!(tst, 363896usize);

        let tst = value.as_u32()?;
        assert_eq!(tst, 363896u32);

        let err = value.as_u16();
        let tst = err.unwrap_err().downcast::<String>().unwrap();
        let eta = format!("{:?}.{:?} expected to be a u16, but: {}", file_path, key, 363896);
        assert_eq!(tst, eta);

        let err = value.as_u8();
        let tst = err.unwrap_err().downcast::<String>().unwrap();
        let eta = format!("{:?}.{:?} expected to be a u8, but: {}", file_path, key, 363896);
        assert_eq!(tst, eta);

        let err = value.as_str();
        let tst = err.unwrap_err().downcast::<String>().unwrap();
        let eta = format!("{:?}.{:?} expected to be a String, but: {}", file_path, key, serde_json::to_string_pretty(&value.value)?);
        assert_eq!(tst, eta);

        let value = json.get([By::key("defaultModificationId")])?;
        let tst = format!("{}", value);
        let eta = r#""test_data/sample.json"."defaultModificationId": 363896"#;
        assert_eq!(tst, eta);
        
        let key = "defaultModificationId2";
        let err = json.get([
            By::key(key)
        ]);
        let tst = err.unwrap_err().downcast::<String>().unwrap();
        let eta = format!("{:?}: not found .{:?} at {}", file_path, key, serde_json::to_string_pretty(&json.value)?);
        assert_eq!(tst, eta); // assert_eq!(&err[0..80], &expected[0..80]);
        
        let value = json.get([By::key("breadcrumbs"), By::index(1), By::key("title")])?;
        let tst = value.as_str()?;
        let eta = "Транспорт";
        assert_eq!(tst, eta);

        let value = json.get([By::key("breadcrumbs")])?;
        let _tst = value.as_vec()?;

        let value = json.get([By::key("breadcrumbs"), By::index(1)])?;
        let _tst = value.as_map()?;

        let value = json.get([By::key("modifications"), By::key("items"), By::index(0), By::key("specification"), By::key("blocks"), By::index(0), By::key("params"), By::index(1), By::key("value")])?;
        let tst = value.as_str()?;
        assert_eq!(tst, "150");

        // let _tst = value.as_map()?;

        Ok(())
    }

}

