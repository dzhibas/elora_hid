use std::{collections::BTreeMap, env, error::Error, time::Duration};

use chrono::{Datelike, Duration as ChronoDuration, Timelike, Utc, Weekday};
use dotenv::dotenv;
use hidapi::{DeviceInfo, HidApi};
use reqwest::Client;
use serde::Deserialize;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8D1D;
/// Elora product id
const PRODUCT_ID: u16 = 0x9D9D;

const USAGE_ID: u16 = 0x61;
const USAGE_PAGE: u16 = 0xFF60;

/// How often to refetch new data when market is open (2 minutes)
const REFRESH_RATE_MARKET_OPEN_SECS: u64 = 2 * 60;
/// How often to refetch new data when market is closed (3 hours)
const REFRESH_RATE_MARKET_CLOSED_SECS: u64 = 3 * 60 * 60;

/// US market open time in UTC (14:30 UTC = 15:30 CET / 3:30 PM CET)
const MARKET_OPEN_HOUR: u32 = 14;
const MARKET_OPEN_MINUTE: u32 = 30;
/// US market close time in UTC (21:00 UTC = 22:00 CET / 10:00 PM CET)
const MARKET_CLOSE_HOUR: u32 = 21;
const MARKET_CLOSE_MINUTE: u32 = 0;

/// HID packet size in bytes
const PACKET_SIZE: usize = 32;

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

/// Checks if the US stock market is currently open
/// Market hours: 14:30-21:00 UTC (15:30-22:00 CET), Monday-Friday
fn is_market_open() -> bool {
    let now = Utc::now();
    let weekday = now.weekday();

    // Market is closed on weekends
    if weekday == Weekday::Sat || weekday == Weekday::Sun {
        return false;
    }

    let current_minutes = now.hour() * 60 + now.minute();
    let open_minutes = MARKET_OPEN_HOUR * 60 + MARKET_OPEN_MINUTE;
    let close_minutes = MARKET_CLOSE_HOUR * 60 + MARKET_CLOSE_MINUTE;

    current_minutes >= open_minutes && current_minutes < close_minutes
}

/// Calculates the next market open time in UTC
fn get_next_market_open() -> chrono::DateTime<Utc> {
    let mut date = Utc::now().date_naive();
    let mut weekday = date.weekday();

    // If Saturday or Sunday, advance to Monday
    if weekday == Weekday::Sat {
        date += ChronoDuration::days(2);
    } else if weekday == Weekday::Sun {
        date += ChronoDuration::days(1);
    }

    // Now date is Monday to Friday
    let now = Utc::now();
    let open_today = date
        .and_hms_opt(MARKET_OPEN_HOUR, MARKET_OPEN_MINUTE, 0)
        .unwrap()
        .and_utc();

    if now < open_today {
        open_today
    } else {
        // After today's open, so next is tomorrow
        date += ChronoDuration::days(1);
        weekday = date.weekday();
        if weekday == Weekday::Sat {
            date += ChronoDuration::days(2);
        } else if weekday == Weekday::Sun {
            date += ChronoDuration::days(1);
        }
        date.and_hms_opt(MARKET_OPEN_HOUR, MARKET_OPEN_MINUTE, 0)
            .unwrap()
            .and_utc()
    }
}

/// Returns the appropriate refresh rate based on market hours
fn get_refresh_rate() -> Duration {
    if is_market_open() {
        log::debug!(
            "Market is open, using {} second refresh rate",
            REFRESH_RATE_MARKET_OPEN_SECS
        );
        Duration::from_secs(REFRESH_RATE_MARKET_OPEN_SECS)
    } else {
        let next_open = get_next_market_open();
        let until_open = next_open.signed_duration_since(Utc::now());
        if until_open < ChronoDuration::hours(3) {
            let delay = (until_open + ChronoDuration::minutes(2)).to_std().unwrap();
            log::info!(
                "Market closed, time until open < 3 hours, delaying {} seconds",
                delay.as_secs()
            );
            delay
        } else {
            log::info!(
                "Market is closed, using {} second refresh rate",
                REFRESH_RATE_MARKET_CLOSED_SECS
            );
            Duration::from_secs(REFRESH_RATE_MARKET_CLOSED_SECS)
        }
    }
}

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
    let mut s = String::new();
    let mut first = true;
    for (ticker, v) in stocks {
        // we use max 4 chars for ticker so it fits. example:
        // TSLA: 500$
        // VWRL: 200$
        if !first {
            s.push('\n');
        }
        first = false;
        s.push_str(&format!("{:.4}: {:.0}$", ticker, v));
    }
    let mut buf = s.into_bytes();
    buf.resize(PACKET_SIZE, 0);
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

    let debug_str: String = buf
        .iter()
        .map(|&b| {
            if (32..=126).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect();
    log::debug!("Buffer sent to HID device: {}", debug_str);

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
    dotenv().ok();
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

    loop {
        let _ = run().await;
        // Recalculate refresh rate based on market hours
        let refresh_rate = get_refresh_rate();
        let mut interval = tokio::time::interval(refresh_rate);
        interval.reset();
        interval.tick().await;
    }
}

#[tokio::test]
async fn testing_fetch_of_stock() -> Result<(), AppError> {
    dotenv().ok();
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

    assert!(st.contains_key("TSLA"));
    assert!(st.get("TSLA").unwrap() > &0.0);

    assert!(st.contains_key("NVDA"));
    assert!(st.get("NVDA").unwrap() > &0.0);

    dbg!(&st);

    Ok(())
}

#[test]
fn testing_conversion_to_buffer() {
    let stocks: StockTickerType = BTreeMap::from([("TSLA", 500.0), ("NVDA", 200.1)]);
    let buf = convert_to_buffer(stocks);
    assert_eq!(buf.len(), PACKET_SIZE);
    let string_part = buf.into_iter().take_while(|&x| x != 0).collect::<Vec<u8>>();
    assert_eq!(
        String::from_utf8(string_part).unwrap(),
        "NVDA: 200$\nTSLA: 500$"
    );
}
