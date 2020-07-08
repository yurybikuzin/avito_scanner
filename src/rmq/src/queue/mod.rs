pub mod request;
pub mod cmd;
pub mod proxies_to_check;

const STATE_PROXIES_TO_CHECK_NONE: u8 = 0;
const STATE_PROXIES_TO_CHECK_STARTED: u8 = 1;
const STATE_PROXIES_TO_CHECK_FILLED: u8 = 2;
use std::sync::atomic::{AtomicU8, Ordering};
pub static STATE_PROXIES_TO_CHECK: AtomicU8 = AtomicU8::new(STATE_PROXIES_TO_CHECK_NONE);

const STATE_PROXIES_TO_USE_NONE: u8 = 0;
const STATE_PROXIES_TO_USE_FILLED: u8 = 2;
pub static STATE_PROXIES_TO_USE: AtomicU8 = AtomicU8::new(STATE_PROXIES_TO_USE_NONE);
