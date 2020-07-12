pub mod request;
pub mod cmd;
pub mod proxies_to_check;
use std::sync::atomic::{Ordering, AtomicU8, AtomicU16};

const STATE_PROXIES_TO_CHECK_NONE: u8 = 0;
const STATE_PROXIES_TO_CHECK_STARTED: u8 = 1;
const STATE_PROXIES_TO_CHECK_FILLED: u8 = 2;
pub static STATE_PROXIES_TO_CHECK: AtomicU8 = AtomicU8::new(STATE_PROXIES_TO_CHECK_NONE);

const STATE_PROXIES_TO_USE_NONE: u8 = 0;
const STATE_PROXIES_TO_USE_FILLED: u8 = 2;
pub static STATE_PROXIES_TO_USE: AtomicU8 = AtomicU8::new(STATE_PROXIES_TO_USE_NONE);

// ======================================

pub static PROXY_TIMEOUT: AtomicU8 = AtomicU8::new(10);//secs
pub static OWN_IP_FRESH_DURATION: AtomicU8 = AtomicU8::new(10);//secs
pub static SAME_TIME_PROXY_CHECK_MAX: AtomicU16 = AtomicU16::new(20);
pub static SAME_TIME_REQUEST_MAX: AtomicU16 = AtomicU16::new(50);
pub static SUCCESS_COUNT_MAX: AtomicU8 = AtomicU8::new(5);
pub static SUCCESS_COUNT_START: AtomicU8 = AtomicU8::new(2);

pub static RESPONSE_TIMEOUT: AtomicU8 = AtomicU8::new(10);//secs
pub static PROXY_REST_DURATION: AtomicU16 = AtomicU16::new(1000);//millis

