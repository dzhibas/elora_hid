use std::{collections::HashMap, ffi::CString, time::Duration};

use hidapi::{HidApi, HidError};
use regex::Regex;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;
/// Elora product id
const PRODUCT_ID: u16 = 0x9d9d;

const USAGE_ID: u16 = 0x61;
const USAGE_PAGE: u16 = 0xFF60;

/// How often to refetch new data from dependency services
const REFRESH_RATE_SECS: u16 = 60;

async fn fetch_stock_tickers() -> HashMap<&'static str, f64> {
    println!("Run of stock tickers function");

    let mut stocks = HashMap::from([("TSLA", 0.0), ("VWRL", 0.0)]);

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
    stocks.clone()
}

fn convert_to_buffer(stocks: HashMap<&'static str, f64>) -> Vec<u8> {
    let mut buf = Vec::new();
    for (ticker, v) in stocks {
        let st_string = format!("{}: {}", ticker, v);
        for ch in st_string.chars() {
            buf.push(ch as u8);
        }
    }
    buf
}

async fn send_to_keyboard(
    keyboard: &CString,
    stocks: HashMap<&'static str, f64>,
) -> Result<(), HidError> {
    println!("Sending to usb keyboard");
    let api = HidApi::new()?;
    let device_info = api
        .device_list()
        .find(|&d| d.path().to_owned() == *keyboard);
    let device = device_info.unwrap().open_device(&api);

    let buf = convert_to_buffer(stocks);
    device?.write(&buf)?;

    Ok(())
}

async fn run(keyboard: &CString) {
    let stocks = fetch_stock_tickers().await;
    let _ = send_to_keyboard(&keyboard, stocks).await;
}

#[tokio::main]
async fn main() {
    let interface: Option<CString> = match HidApi::new() {
        Ok(api) => {
            let mut found: Option<CString> = None;
            for dev in api.device_list() {
                if dev.vendor_id() == VENDOR_ID
                    && dev.product_id() == PRODUCT_ID
                    && dev.usage() == USAGE_ID
                    && dev.usage_page() == USAGE_PAGE
                {
                    println!(
                        "{:03x}:{:04x} {:?} {:?}",
                        dev.vendor_id(),
                        dev.product_id(),
                        dev.manufacturer_string(),
                        dev.product_string()
                    );
                    found = Some(dev.path().to_owned());
                    break;
                }
            }
            found
        }
        Err(_) => None,
    };

    if interface.is_none() {
        eprintln!("Keyboard not found connected");
        return ();
    }

    let mut interval = tokio::time::interval(Duration::from_secs(REFRESH_RATE_SECS.into()));
    loop {
        interval.tick().await;
        run(&interface.clone().unwrap()).await;
    }
}

#[tokio::test]
async fn testing_fetch_of_stock() {
    let st = fetch_stock_tickers().await;
    assert_eq!(st.contains_key("TSLA"), true);
    assert_eq!(st.get(&"TSLA").unwrap() > &0.0, true);

    assert_eq!(st.contains_key("VWRL"), true);
}
