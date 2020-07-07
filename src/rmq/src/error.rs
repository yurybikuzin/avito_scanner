
use deadpool_lapin::PoolError;
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("rmq error: {0}")]
    // RMQError(#[from] lapin::Error),
    RMQError(#[from] anyhow::Error),
    #[error("rmq pool error: {0}")]
    RMQPoolError(#[from] PoolError),
}
