#![recursion_limit="2048"]

#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use futures::{join };
use std::env;

mod api;
mod handlers;
// mod rmq;
mod error;
mod queue;
//
#[tokio::main]
async fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        // Set `RUST_LOG=todos=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "proxy=trace");
    }
    pretty_env_logger::init();

    let pool = rmq::get_pool()?;

    let routes = api::api(pool.clone());

    let port = env::var("RMQ_PORT").unwrap_or("8000".to_owned()).parse::<u16>()?;
    println!("Started server at localhost:{}", port);
    let _ = join!(
        warp::serve(routes).run(([0, 0, 0, 0], port)),
        queue::cmd::process(pool.clone()),
        queue::proxies_to_check::process(pool.clone()),
        queue::request::process(pool.clone()),
    );
    Ok(())
}




