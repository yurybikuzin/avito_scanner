#![recursion_limit="512"]

#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use futures::{join };
use std::env;

mod req;
mod api;
mod handlers;
mod rmq;
mod error;
mod queue;

#[tokio::main]
async fn main() -> Result<()> {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=todos=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "rmq=info");
    }
    pretty_env_logger::init();

    let pool = rmq::get_pool();

    let routes = api::api(pool.clone());

    println!("Started server at localhost:8000");
    let _ = join!(
        warp::serve(routes).run(([0, 0, 0, 0], 8000)),
        queue::request::process(pool.clone()),
        queue::fetch_proxies::process(pool.clone()),
        queue::proxies_to_check::process(pool.clone()),
    );
    Ok(())
}




