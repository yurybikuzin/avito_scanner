
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

// use std::env;
use config::{ConfigError, Config, File, Environment};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    user: String,
    pass: String,
    host: String,
    port: u16,
    vhost: String,
}

impl Settings {
    pub fn addr(&self) -> String {
        format!("amqp://{}:{}@{}:{}/{}", self.user, self.pass, self.host, self.port, self.vhost)
    }
}

// use std::sync::RwLock;
// lazy_static::lazy_static!{
//     pub static ref SINGLETON: RwLock<Option<Settings>> = RwLock::new(None);
// }

use std::path::Path;
impl Settings {
    pub fn new(source: &Path) -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Start off by merging in the "default" configuration file
        s.merge(File::with_name(&source.to_string_lossy()))?;

        // Add in the current environment file
        // Default to 'development' env
        // Note that this file is _optional_
        // let env = env::var("RUN_MODE").unwrap_or("development".into());
        // s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        // s.merge(File::with_name("config/local").required(false))?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("rmq"))?;

        // You may also programmatically change settings
        // s.set("database.url", "postgres://")?;

        // Now that we're done, let's access our configuration
        // println!("debug: {:?}", s.get_bool("debug"));
        // println!("database: {:?}", s.get::<String>("database.url"));

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into()
    }
    pub fn as_string_pretty(&self) -> Result<String> {
        let s = serde_json::to_string_pretty(&self)?;
        Ok(s)
    }
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
