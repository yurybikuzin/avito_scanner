
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use std::io::{stdout , Write};

use crossterm::{
    queue,
    terminal, 
    cursor,
    style::Print,
};

// ============================================================================
// ============================================================================

pub struct Term {
    stdout: TermStdout,
    anchors: TermAnchors,
}

impl Term {
    pub fn init(s: String) -> Result<Self> {
        let mut stdout = TermStdout::new();
        let anchors = stdout.output(s)?;
        Ok(Self {
            stdout,
            anchors,
        })
    }
    pub fn output(&mut self, s: String) -> Result<()> {
        self.stdout.restore_cursor(&self.anchors)?;
        self.anchors = self.stdout.output(s)?;
        Ok(())
    }
}

// ============================================================================

struct TermAnchors {
    row_prev: u16,
    row_last: u16,
}

// ============================================================================

struct TermStdout {
    stdout: std::io::Stdout,
}

impl TermStdout {
    fn new() -> Self {
        Self {
            stdout: stdout()
        }
    }
    fn output(&mut self, s: String) -> Result<TermAnchors> {
        let mut row_prev = crossterm::cursor::position()?.1;
        let rows = crossterm::terminal::size()?.1;
        if row_prev == rows - 1 {
            row_prev -= 2;
            queue!(self.stdout, 
                terminal::ScrollUp(2),
                cursor::MoveTo(0, row_prev),
            )?;
        }
        queue!(self.stdout,
            Print(s),
            Print("\n".to_owned())
        )?;
        self.stdout.flush()?;
        let row_last = crossterm::cursor::position()?.1;
        Ok(TermAnchors {row_prev, row_last})
    }
    fn restore_cursor(&mut self, anchors: &TermAnchors) -> Result<()> {
        let (col_new, row_new) = crossterm::cursor::position()?;
        if row_new == anchors.row_last && col_new == 0 {
            queue!(self.stdout,
                cursor::MoveTo(0, anchors.row_prev)
            )?;
        }
        Ok(())
    }
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {

    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;
    use std::sync::Once;
    static INIT: Once = Once::new();
    fn init() {
        INIT.call_once(|| env_logger::init());
    }

    #[tokio::test]
    async fn it_works() -> Result<()> {
        init();

        let arg = Arg { 
            qt: 1000,
            warn_chance_treshold: 1,
            info_chance_treshold: 3,
        };
        let mut term = Term::init("ids::get".to_owned())?;
        long_live(&arg, Some(|arg: CallbackArg| -> Result<()> {
            term.output(format!("ids::get: time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
                arrange_millis::get(arg.elapsed_millis), 
                arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
                arrange_millis::get(arg.remained_millis), 
                arrange_millis::get(arg.per_millis), 
                arg.elapsed_qt,
                arg.elapsed_qt + arg.remained_qt,
                arg.remained_qt,
            ))
        })).await?;

        Ok(())
    }

    pub struct Arg {
        pub qt: u64,
        pub warn_chance_treshold: u8,
        pub info_chance_treshold: u8,
    }
    use rand::Rng;

    pub struct CallbackArg {
        pub elapsed_qt: u64,
        pub remained_qt: u64,
        pub elapsed_millis: u128,
        pub remained_millis: u128,
        pub per_millis: u128,
    }

    pub type Ret = ();

    use std::{thread, time::{Duration, Instant}};

    const CALLBACK_THROTTLE: u128 = 100;
    pub async fn long_live<Cb>(
        arg: &Arg, 
        mut callback: Option<Cb>,
    ) -> Result<Ret> 
    where 
        Cb: FnMut(CallbackArg) -> Result<()>,
    {
        let ten_millis = Duration::from_millis(10);
        let mut elapsed_qt = 0;
        let mut remained_qt = arg.qt;
        let start = Instant::now();
        let mut last_callback = Instant::now();
        for _ in 0..remained_qt {
            thread::sleep(ten_millis);
            elapsed_qt += 1;
            if remained_qt > 0 {
                remained_qt -= 1;
            }
            let mut rng = rand::thread_rng();
            let chance: u8 = rng.gen();
            if chance <= arg.warn_chance_treshold {
                warn!("some warning");
            } else if chance <= arg.info_chance_treshold {
                info!("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.");
            }
            callback = if let Some(mut callback) = callback {
                elapsed_qt += 1;
                if remained_qt > 0 {
                    remained_qt -= 1;
                }
                if Instant::now().duration_since(last_callback).as_millis() >= CALLBACK_THROTTLE {
                    let elapsed_millis = Instant::now().duration_since(start).as_millis(); 
                    let per_millis = elapsed_millis / elapsed_qt as u128;
                    let remained_millis = per_millis * remained_qt as u128;
                    callback(CallbackArg {
                        elapsed_qt,
                        remained_qt,
                        elapsed_millis, 
                        remained_millis, 
                        per_millis,
                    })?;
                    last_callback = Instant::now();
                }
                Some(callback)
            } else {
                None
            };
        }
        Ok(())
    }
}
