use std::{collections::HashMap, time::Duration};

use hidapi::HidApi;
use regex::Regex;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;
/// Elora product id
const PRODUCT_ID: u16 = 0x9d9d;

/// How often to refetch new data from dependency services
const REFRESH_RATE_SECS: u16 = 60;

async fn fetch_stock_tickers() {
    println!("Run of stock tickers function");

    let mut stocks = HashMap::from([("TSLA", 0.0), ("APPL", 0.0)]);

    for stock in stocks.clone().into_iter() {
        let regex_str = format!(
            "data-symbol=\"{}.*?regularMarketPrice.*?value=\"(?<price>.*?)\"",
            stock.0
        );

        let price = Regex::new(&regex_str).unwrap();
        let url = format!("https://finance.yahoo.com/quote/{}/", stock.0);
        let req = reqwest::get(url).await;
        let body = req.expect("Request failed").text().await.unwrap();

        if let Some(caps) = price.captures(&body) {
            let b = caps.name("price").map_or("0", |m| m.as_str());
            stocks
                .get_mut(stock.0)
                .map(|v| *v = b.parse().unwrap_or(0.0));
        }
    }

    println!("Fetch done");
}

async fn run() {
    fetch_stock_tickers().await;
}

#[tokio::main]
async fn main() {
    let mut found = false;

    match HidApi::new() {
        Ok(api) => {
            for dev in api.device_list() {
                if dev.vendor_id() == VENDOR_ID && dev.product_id() == PRODUCT_ID {
                    found = true;
                    println!(
                        "{:03x}:{:04x} {:?} {:?}",
                        dev.vendor_id(),
                        dev.product_id(),
                        dev.manufacturer_string(),
                        dev.product_string()
                    );
                    break;
                }
            }
        }
        Err(_) => eprintln!("Error happened"),
    }

    if !found {
        eprintln!("Keyboard not found connected");
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(REFRESH_RATE_SECS.into()));
    loop {
        interval.tick().await;
        run().await;
    }
}
