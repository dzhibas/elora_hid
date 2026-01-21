use std::{collections::BTreeMap, env, error::Error, time::Duration};

use hidapi::{DeviceInfo, HidApi};
use reqwest::Client;
use serde::Deserialize;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;
/// Elora product id
const PRODUCT_ID: u16 = 0x9d9d;

const USAGE_ID: u16 = 0x61;
const USAGE_PAGE: u16 = 0xFF60;

/// How often to refetch new data from dependency services in seconds
const REFRESH_RATE_SECS: u16 = 60;

// type alias for stock tickers
type StockTickerType = BTreeMap<&'static str, f64>;
// interested tickers
const TICKERS: [(&str, f64); 2] = [("TSLA", 0.0), ("NVDA", 0.0)];

/// Finnhub quote response
#[derive(Deserialize)]
struct FinnhubQuote {
    /// Current price
    c: f64,
}

// custom app error
type AppError = Box<dyn Error>;

async fn fetch_stock_tickers() -> Result<StockTickerType, AppError> {
    log::info!("Fetching stock tickers from remote");

    let token = env::var("FINNHUB_TOKEN").expect("FINNHUB_TOKEN environment variable must be set");
    let mut stocks = BTreeMap::from(TICKERS);
    let client = Client::new();

    for stock in stocks.clone().into_iter() {
        let url = format!(
            "https://finnhub.io/api/v1/quote?symbol={}&token={}",
            stock.0, token
        );
        let resp = client.get(&url).send().await?;
        let quote: FinnhubQuote = resp.json().await?;

        if let Some(v) = stocks.get_mut(stock.0) {
            *v = quote.c;
        }
    }

    log::debug!("Fetching complete");

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

/// searches for connected elora keyboard
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
    log::info!("Sending to usb keyboard");

    let api = HidApi::new()?;
    let device = find_elora_device(&api);

    if device.is_none() {
        return Err("Device disconnected".into());
    }

    let device = device.unwrap().open_device(&api);
    let buf = convert_to_buffer(stocks);
    device?.write(&buf)?;

    log::debug!("{}", String::from_utf8(buf).unwrap());

    Ok(())
}

/// Main worker which fetches stuff and sends it to keyboard
async fn run() -> Result<(), AppError> {
    let stocks = fetch_stock_tickers().await?;
    let res = send_to_keyboard(stocks).await;
    if res.is_err() {
        log::error!("Error occured while sending data to keyboard");
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

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
        log::error!("Error: Elora keyboard not found connected");
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
    // Skip test if FINNHUB_TOKEN is not set
    if env::var("FINNHUB_TOKEN").is_err() {
        eprintln!("Skipping test: FINNHUB_TOKEN not set");
        return Ok(());
    }

    let st = fetch_stock_tickers().await?;

    // Example output:
    //
    // [src/main.rs:120] &st = {
    // "NVDA": 130.5,
    // "TSLA": 419.25,
    // }

    assert_eq!(st.contains_key("TSLA"), true);
    assert_eq!(st.get("TSLA").unwrap() > &0.0, true);

    assert_eq!(st.contains_key("NVDA"), true);
    assert_eq!(st.get("NVDA").unwrap() > &0.0, true);

    dbg!(&st);

    Ok(())
}

#[test]
fn testing_conversion_to_buffer() {
    let stocks: StockTickerType = BTreeMap::from([("TSLA", 500.0), ("NVDA", 200.1)]);
    let buf = convert_to_buffer(stocks);
    assert_eq!(String::from_utf8(buf).unwrap(), "NVDA: 200$TSLA: 500$");
}
