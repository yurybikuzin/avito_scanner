
#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow, Context};

use std::path::{Path, PathBuf};

pub fn get(out_dir: &Path, id: u64) -> PathBuf {
    let s = format!("{:016x}", id);
    // https://users.rust-lang.org/t/solved-how-to-split-string-into-multiple-sub-strings-with-given-length/10542
    let path_vec = s
        .as_bytes()
        .chunks(2)
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
    ;
    let path_vec: Vec<&Path> = path_vec.iter().map(|item| Path::new(item)).collect();
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
        assert_eq!(fspec.to_string_lossy(), "out_test/ff/ff/ff/ff/ff/ff/ff/fe.json");

        let out_dir = &Path::new("out");
        let id = std::u64::MAX;
        let fspec = get(out_dir, id);
        assert_eq!(fspec.to_string_lossy(), "out/ff/ff/ff/ff/ff/ff/ff/ff.json");

        Ok(())
    }

}



