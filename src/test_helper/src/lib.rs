use std::sync::Once;
static INIT: Once = Once::new();
// std::sync::atomic::{AtomicU8, Ordering};
//
// static COUNT: AtomicU8 = AtomicU8::new(0);

// fn init_logger() {
//     env_logger::init()
//     // INIT.call_once(|| pretty_env_logger::init());
//     // INIT.call_once(|| );
// }
pub fn init() {
    // println!("COUNT", COUNT);
    // if COUNT.
    // COUNT.fetch_add(1, Ordering::SegCst);

    INIT.call_once(|| pretty_env_logger::init());
    // INIT.call_once(|| env_logger::init());
    // INIT.call_once(init_logger);
}

// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         assert_eq!(2 + 2, 4);
//     }
// }
