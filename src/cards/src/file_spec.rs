
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path, PathBuf};

pub fn get(out_dir: &Path, id: u64) -> PathBuf {
    let s = format!("{:016x}", id);
    let ss = vec![ "cards", &s[0..9], &s[9..16] ];
    let path_vec: Vec<&Path> = ss.into_iter().map(|item| Path::new(item)).collect();
    let path: PathBuf = [ out_dir ]
        .iter()
        .chain(path_vec.iter())
        .collect()
    ;
    path.with_extension("json")
}

// ============================================================================
// ============================================================================
// ============================================================================

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use log::{error, warn, info, debug, trace};
    use super::*;

    #[tokio::test]
    async fn test_file_spec() -> Result<()> {
        test_helper::init();

        let out_dir = &Path::new("out_test");

        let id = std::u64::MAX - 1;
        let fspec = get(out_dir, id);
        assert_eq!(fspec.to_string_lossy(), "out_test/cards/fffffffff/ffffffe.json");

        let out_dir = &Path::new("out");
        let id = std::u64::MAX;
        let fspec = get(out_dir, id);
        assert_eq!(fspec.to_string_lossy(), "out/cards/fffffffff/fffffff.json");

        Ok(())
    }

}



