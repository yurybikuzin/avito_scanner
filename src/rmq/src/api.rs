#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use super::rmq::Pool;
use warp::Filter;
use super::handlers;
use std::convert::Infallible;
use warp::{Rejection, Reply};

pub fn api(
    pool: Pool,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    health_route()
        .or(add_msg_route(pool))
}

/// GET /health
pub fn health_route() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("health").and_then(handlers::health)
}

/// POST /req
pub fn add_msg_route(pool: Pool) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("req")
        .and(warp::post())
        // .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(with_rmq(pool.clone()))
        .and_then(handlers::req)
}

fn with_rmq(pool: Pool) -> impl Filter<Extract = (Pool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}
