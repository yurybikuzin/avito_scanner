#![recursion_limit="2048"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use futures::{join };
use std::env;

use structopt::StructOpt;
use std::path::PathBuf;
#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    config: PathBuf,
}
mod settings;
use settings::{Settings, SINGLETON};

mod api;
mod handlers;
mod error;
mod queue;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init_timed();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    let opt = Opt::from_args();

    let settings = Settings::new(&opt.config).map_err(|err| anyhow!("{:?}: {}", opt.config, err))?;
    {
        let mut singleton = SINGLETON.write().unwrap(); 
        println!("config: {:?}, settings: {}", opt.config, settings.as_string_pretty()?);
        (*singleton).replace(settings);
    }

    let pool = rmq::get_pool()?;

    let routes = api::api();

    let port = env::var("PROXY_PORT").unwrap_or("8000".to_owned()).parse::<u16>()?;

    let _ = join!(
        warp::serve(routes).run(([0, 0, 0, 0], port)),
        queue::process(pool.clone()),
    );
    Ok(())
}




