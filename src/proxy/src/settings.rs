
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

// use std::env;
use config::{ConfigError, Config, File, 
    // Environment
};
use serde::{Serialize, Deserialize};
// use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub sources: Vec<String>,
    pub proxy_timeout_secs: u64,
    pub own_ip_fresh_duration_secs: u64,
    pub same_time_proxy_check_max_count: usize,
    pub same_time_request_max_count: usize,
    pub response_timeout_secs: u64,
    pub proxy_rest_duration_millis: u64,
    pub success_start_count: usize,
    pub success_max_count: usize,
    // pub own_ip: Option<String>,
    pub echo_service_url: String,
}

lazy_static::lazy_static!{
    // static ref CONTENT_LENGTH_LIMIT: Mutex<usize> = Mutex::new(0);
    pub static ref SINGLETON: RwLock<Option<Settings>> = RwLock::new(None);
}

// pub static PROXY_TIMEOUT: AtomicU8 = AtomicU8::new(45);//secs
// pub static OWN_IP_FRESH_DURATION: AtomicU8 = AtomicU8::new(10);//secs
// pub static SAME_TIME_PROXY_CHECK_MAX: AtomicUsize = AtomicUsize::new(50);
//
// pub static SAME_TIME_REQUEST_MAX: AtomicUsize = AtomicUsize::new(50);
// pub static RESPONSE_TIMEOUT: AtomicU8 = AtomicU8::new(45);//secs
// pub static PROXY_REST_DURATION: AtomicU16 = AtomicU16::new(1000);//millis
//
// pub static SUCCESS_COUNT_START: AtomicU8 = AtomicU8::new(2);
// pub static SUCCESS_COUNT_MAX: AtomicU8 = AtomicU8::new(5);

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
        // s.merge(Environment::with_prefix("app"))?;

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
