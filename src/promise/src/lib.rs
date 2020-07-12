use std::sync::{Mutex, Arc};
// use futures::future::{FutureExt};

#[derive(Debug, Clone)]
pub struct Promise<T> where T: Sized {
    shared_state: Arc<Mutex<SharedState<T>>>,
}

impl<T> Promise<T> {
    pub fn new() -> Self {
        Self {
            shared_state: Arc::new(Mutex::new(SharedState {
                ret: None,
                waker: None,
            })),
        }
    }
    pub fn resolve(&mut self, ret: T) {
        let mut shared_state = self.shared_state.lock().unwrap();
        match shared_state.ret {
            Some(_) => unreachable!(),
            None => {
                shared_state.ret.replace(ret);
                if let Some(waker) = shared_state.waker.take() {
                    waker.wake()
                }
            },
        }
    }
}

// impl<T: ?Sized> FutureExt for T where T: Promise {}

// impl<T> std::marker::Sized for Promise<T> {}
// impl<T> FutureExt for Promise<T> {}

impl<T> futures::Future for Promise<T> {
    type Output = T;
    // https://rust-lang.github.io/async-book/02_execution/03_wakeups.html
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context) -> std::task::Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if let Some(ret) = shared_state.ret.take() {
            std::task::Poll::Ready(ret)
        } else {
            shared_state.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

#[derive(Debug, Clone)]
struct SharedState<T> {
    ret: Option<T>,
    waker: Option<std::task::Waker>,
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use anyhow::{anyhow, bail, Result, Error, Context};
    use futures::executor::ThreadPool;

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| pretty_env_logger::init());
    }

    #[tokio::test]
    async fn test_promise() -> Result<()> {
        init();

        let promise = Promise::new();
        let mut promise_clone = promise.clone();
        let future = async move {
            let mut promise_err = promise_clone.clone();
            let future = async move {
                promise_clone.resolve("OK");
                Ok::<(), Error>(())
            };
            if let Err(err) = future.await {
                error!("future: {}", err);
                promise_err.resolve("Err");
            }
        };
        let pool = ThreadPool::new().unwrap();
        pool.spawn_ok(future);
        let ret = promise.await;
        assert_eq!(ret, "OK");

        let promise = Promise::new();
        #[allow(unused_mut)]
        let mut promise_clone = promise.clone();
        let future = async move {
            let mut promise_err = promise_clone.clone();
            let future = async move {
                bail!("some expected err");
                promise_clone.resolve("OK");
                Ok::<(), Error>(())
            };
            if let Err(err) = future.await {
                error!("future: {}", err);
                promise_err.resolve("Err");
            }
        };
        let pool = ThreadPool::new().unwrap();
        pool.spawn_ok(future);
        let ret = promise.await;
        assert_eq!(ret, "Err");

        Ok(())
    }

}
