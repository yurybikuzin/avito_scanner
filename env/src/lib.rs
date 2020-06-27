#[allow(unused_imports)]
use log::{error, warn, info, debug, trace};
#[allow(unused_imports)]
use anyhow::{Result, Error, bail, anyhow};

/// Возвращает значение переменной среды env_var_name
/// Если переменная среды не определена, возвращает значение по умолчанию default
///
/// #Errors
///
/// Если значение переменноё среды определено, но не удалось преобразовать в тип T, возвращает
/// ошибку
pub fn get<T: std::str::FromStr>(env_var_name: &str, default: T) -> Result<T> {
    std::env::var(env_var_name)
        .map_or_else(
            |_err| Ok(default), 
            |val| val.parse::<T>().map_err(
                |_err| anyhow!("failed to parse {}='{}'", env_var_name, val)
            ),
        )
}

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

    const COUNT_LIMIT: u64 = 4900;
    const PRICE_PRECISION: isize = 20000;
    const AUTH: &str = "af0deccbgcgidddjgnvljitntccdduijhdinfgjgfjir";

    #[tokio::test]
    async fn it_works() -> Result<()> {
        init();

        let count_limit: u64 = get("AVITO_COUNT_LIMIT", COUNT_LIMIT)?;
        let price_precision: isize = get("AVITO_PRICE_PRECISION", PRICE_PRECISION)?;
        let auth: String = get("AVITO_AUTH", AUTH.to_owned())?;
        info!("count_limit: {}, price_precision: {}, auth: {}", count_limit, price_precision, auth);

        Ok(())
    }

}
