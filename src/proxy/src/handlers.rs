#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use warp::{Rejection, Reply};
type WebResult<T> = std::result::Result<T, Rejection>;
use super::error::{self};

impl warp::reject::Reject for error::Error {}

pub async fn health() -> WebResult<impl Reply> {
    Ok("OK")
}

