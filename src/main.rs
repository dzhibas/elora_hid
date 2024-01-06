use std::{collections::BTreeMap, error::Error, time::Duration};

use hidapi::{DeviceInfo, HidApi};
use regex::Regex;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;
/// Elora product id
const PRODUCT_ID: u16 = 0x9d9d;

const USAGE_ID: u16 = 0x61;
const USAGE_PAGE: u16 = 0xFF60;

/// How often to refetch new data from dependency services in seconds
const REFRESH_RATE_SECS: u16 = 10;

// type alias for stock tickers
type StockTickerType = BTreeMap<&'static str, f64>;
// interested tickers
const TICKERS: [(&str, f64); 2] = [("TSLA", 0.0), ("VWRL.AS", 0.0)];

// custom app error
type AppError = Box<dyn Error>;

async fn fetch_stock_tickers() -> Result<StockTickerType, AppError> {
    println!("Run of stock tickers function");

    let mut stocks = BTreeMap::from(TICKERS);

    for stock in stocks.clone().into_iter() {
        let regex_str = format!(
            "data-symbol=\"{}.*?regularMarketPrice.*?value=\"(?<price>.*?)\"",
            stock.0
        );

        let price = Regex::new(&regex_str)?;
        let url = format!("https://finance.yahoo.com/quote/{}/", stock.0);
        let req = reqwest::get(url).await?;
        let body = req.text().await?;

        if let Some(caps) = price.captures(&body) {
            let b = caps.name("price").map_or("0", |m| m.as_str());
            if let Some(v) = stocks.get_mut(stock.0) {
                *v = b.parse().unwrap_or(0.0);
            }
        }
    }

    Ok(stocks)
}

/// Converts StockTickerType into string which is sent through usb to keyboard
fn convert_to_buffer(stocks: StockTickerType) -> Vec<u8> {
    let mut buf = Vec::new();
    for (ticker, v) in stocks {
        // we use max 4 chars for ticker so it fits. example:
        // TSLA: 500$
        // VWRL: 200$
        let st_string = format!("{:.4}: {:.0}$", ticker, v);
        for ch in st_string.chars() {
            buf.push(ch as u8);
        }
    }
    buf
}

fn find_elora_device(api: &HidApi) -> Option<&DeviceInfo> {
    let device = api.device_list().find(|&dev| {
        dev.vendor_id() == VENDOR_ID
            && dev.product_id() == PRODUCT_ID
            && dev.usage() == USAGE_ID
            && dev.usage_page() == USAGE_PAGE
    });
    device
}

/// sends stock ticker to keyboard
async fn send_to_keyboard(stocks: StockTickerType) -> Result<(), AppError> {
    println!("Sending to usb keyboard");

    let api = HidApi::new()?;
    let device = find_elora_device(&api);

    if device.is_none() {
        return Err("Device disconnected".into());
    }

    let device = device.unwrap().open_device(&api);
    let buf = convert_to_buffer(stocks);
    device?.write(&buf)?;

    println!("{}", String::from_utf8(buf).unwrap());
    Ok(())
}

/// Main worker which fetches stuff and sends it to keyboard
async fn run() -> Result<(), AppError> {
    let stocks = fetch_stock_tickers().await?;
    let res = send_to_keyboard(stocks).await;
    if res.is_err() {
        eprintln!("Error occured while sending data to keyboard");
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    println!(
        r"
  _____ _                   _   _ ___ ____  
 | ____| | ___  _ __ __ _  | | | |_ _|  _ \ 
 |  _| | |/ _ \| '__/ _` | | |_| || || | | |
 | |___| | (_) | | | (_| | |  _  || || |_| |
 |_____|_|\___/|_|  \__,_| |_| |_|___|____/
"
    );

    let api = HidApi::new().unwrap();
    let device = find_elora_device(&api);

    if device.is_none() {
        eprintln!("Error: Elora keyboard not found connected");
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(REFRESH_RATE_SECS.into()));
    loop {
        interval.tick().await;
        let _ = run().await;
    }
}

#[tokio::test]
async fn testing_fetch_of_stock() -> Result<(), AppError> {
    let st = fetch_stock_tickers().await?;

    // Example output:
    //
    // [src/main.rs:120] &st = {
    // "VWRL.AS": 107.2,
    // "TSLA": 237.03,
    // "AAPL": 180.51,
    // }

    assert_eq!(st.contains_key("TSLA"), true);
    assert_eq!(st.get("TSLA").unwrap() > &0.0, true);

    assert_eq!(st.contains_key("VWRL.AS"), true);
    assert_eq!(st.get("VWRL.AS").unwrap() > &0.0, true);
    Ok(())
}

#[test]
fn testing_conversion_to_buffer() {
    let stocks: StockTickerType = BTreeMap::from([("TSLA", 500.0), ("VWRL.AS", 200.1)]);
    let buf = convert_to_buffer(stocks);
    assert_eq!(String::from_utf8(buf).unwrap(), "TSLA: 500$VWRL: 200$");
}
