
    // // let re = regex::Regex::new(r"My Public IPv4 is: <[^>]+>([^<]+)").unwrap();
    // let re = regex::Regex::new(r#""ipv4":\{"value":"([^"]+)"#).unwrap();
    //
    // let url = format!("https://yandex.ru/internet/");
    // let url = Url::parse(&url)?;
    // // let client = reqwest::Client::new();
    // //
    // // let proxy_http_url = env::get("PROXY_HTTP_URL")?;
    // // let proxy_https_url = env::get("PROXY_HTTPS_URL")?;
    // let client = reqwest::Client::builder()
    //     // .proxy(reqwest::Proxy::http("http://91.235.33.79")?)
    //     .proxy(reqwest::Proxy::all("https://91.235.33.91")?)
    //     // .proxy(reqwest::Proxy::https("https://169.57.1.85")?)
    //     .build()?
    // ;
    // let text = client.get(url).send().await?.text().await?;
    // match re.captures(&text) {
    //     None => info!("ip not found"),
    //     Some(cap) => info!("ip: {}", &cap[1]),
    // }

fn main() {
    println!("Hello, world!");
}
