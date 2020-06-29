#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
use super::*;
use std::sync::Once;
static INIT: Once = Once::new();
fn init() {
    INIT.call_once(|| env_logger::init());
}

use std::collections::HashSet;

#[tokio::test]
async fn test_file_spec() -> Result<()> {
    init();

    let out_dir = &Path::new("out_test");

    let id = std::u64::MAX - 1;
    let fspec = file_spec::get(out_dir, id);
    assert_eq!(fspec.to_string_lossy(), "out_test/ff/ff/ff/ff/ff/ff/ff/fe.json");

    let out_dir = &Path::new("out");
    let id = std::u64::MAX;
    let fspec = file_spec::get(out_dir, id);
    assert_eq!(fspec.to_string_lossy(), "out/ff/ff/ff/ff/ff/ff/ff/ff.json");

    Ok(())
}

#[tokio::test]
async fn test_check() -> Result<()> {
    init();

    let out_dir = &Path::new("out_test");
    let id = 42;

    let ret = check::run(check::Arg { out_dir, id }).await?;
    assert_eq!(ret, check::Ret{id: Some(id)});

    Ok(())
}

#[tokio::test]
async fn test_fetch_and_save() -> Result<()> {
    init();

    let mut ids: ids::Ret = HashSet::new();
    let ids_vec: Vec<u64> = vec![
  1266979170,
  1267406333,
  1267834120,
  1268326195,
  1268369412,
  1268744630,
  1269391469,
  1269500991,
  1269534533,
  1269926878,
  1269933390,
  1271210361,
  1271667415,
  1271807067,
  1272751016,
    ];
    for id in ids_vec {
        ids.insert(id);
    }
    let out_dir = &Path::new("out_test");
    let arg = Arg {
        get_auth,
        ids: &ids,
        out_dir,
        thread_limit_network: 1,
        thread_limit_file: 12,
        retry_count: 3,
    };

    let now = Instant::now();
    let cmd = ansi_escapes::CursorShow;
    println!("cards::fetch_and_save");
    fetch_and_save(&arg, Some(&|arg: CallbackArg| {
        println!("{}ids::get: time: {}/{}-{}, per: {}, qt: {}/{}-{}", 
            cmd,
            arrange_millis::get(arg.elapsed_millis), 
            arrange_millis::get(arg.elapsed_millis + arg.remained_millis), 
            arrange_millis::get(arg.remained_millis), 
            arrange_millis::get(arg.per_millis), 
            arg.elapsed_qt,
            arg.elapsed_qt + arg.remained_qt,
            arg.remained_qt,
        );
    })).await?;
    println!("{}", cmd);
    info!("{}, cards::fetch_and_save", arrange_millis::get(Instant::now().duration_since(now).as_millis()));

    let now = Instant::now();
    info!("cards::fetch_and_save again");
    fetch_and_save(&arg, None).await?;
    info!("{}, cards::fetch_and_save", arrange_millis::get(Instant::now().duration_since(now).as_millis()));

    Ok(())
}

async fn get_auth() -> Result<String> {
    println!("Получение токена авторизации . . .");
    let start = Instant::now();
    let auth = auth::get().await?;
    println!("ТОкен авторизации получен, {}", arrange_millis::get(Instant::now().duration_since(start).as_millis()));
    Ok(auth)
}