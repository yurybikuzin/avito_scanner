
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{anyhow, bail, Result, Error, Context};

use tokio::fs;
use std::path::Path;

// ============================================================================
// ============================================================================

pub struct Arg<'a> {
    pub file_path: &'a Path,
    pub records: &'a collect::Records,
}

pub async fn write<'a>(arg: Arg<'a>) -> Result<()> {
    if let Some(dir_path) = arg.file_path.parent() {
        fs::create_dir_all(dir_path).await?;
    }
    let mut wtr = csv::Writer::from_path(arg.file_path)?;

    for record in &arg.records.0 {
        wtr.serialize(record)?;
    }
    wtr.flush()?;
    Ok(())
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
    async fn test_to_csv() -> Result<()> {
        init();

        Ok(())
    }

}
