
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

// use std::env;
use config::{ConfigError, Config, File, Environment};
use serde::{Serialize, Deserialize};
// use std::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub out_dir: String,
    pub auth_key: Option<String>,
    pub auth_url: Option<String>,
    pub params: String,
    pub count_limit: u64,
    pub diaps_count: usize,
    // pub price_precision: isize,
    pub price_max_inc: isize,
    pub id_fresh_duration_mins: i64,
    pub thread_limit_network: usize,
    pub thread_limit_file: usize,
    pub diap_fresh_duration_mins: i64,
    pub items_per_page: usize,

    // pub sources: Vec<String>,
    // pub proxy_timeout_secs: u64,
    // pub own_ip_fresh_duration_secs: u64,
    // pub same_time_proxy_check_max_count: usize,
    // pub same_time_request_max_count: usize,
    // pub response_timeout_secs: u64,
    // pub proxy_rest_duration_millis: u64,
    // pub success_start_count: usize,
    // pub success_max_count: usize,
    // pub echo_service_url: String,
}

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
        s.merge(Environment::with_prefix("ps"))?;

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
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
