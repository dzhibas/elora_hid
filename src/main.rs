use std::{env, error::Error, path::Path, time::Duration};

use chrono::{Datelike, Duration as ChronoDuration, Timelike, Utc, Weekday};
use dotenv::dotenv;
use hidapi::{DeviceInfo, HidApi};
use tokio::fs;
use tokio::process::Command;

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

/// File paths and Commands
const PRICES_FILE_PATH: &str = "prices.txt";
const PULL_COMMAND: &str = "npx";
const PULL_ARGS: [&str; 2] = ["tsx", "pull.ts"];

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

/// Runs the local command to pull prices and reads the resulting file
async fn fetch_prices_from_local_command() -> Result<String, AppError> {
    log::info!(
        "Executing local command: {} {}",
        PULL_COMMAND,
        PULL_ARGS.join(" ")
    );

    // 1. Run the command
    // Note: On Windows you might need "npx.cmd" depending on your environment,
    // but usually "npx" works in modern shells.
    let status = Command::new(PULL_COMMAND)
        .args(PULL_ARGS)
        .status()
        .await
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    if !status.success() {
        return Err(format!("Command finished with non-zero exit code: {}", status).into());
    }

    // 2. Check if file exists
    if !Path::new(PRICES_FILE_PATH).exists() {
        return Err(format!("Command finished, but {} was not found", PRICES_FILE_PATH).into());
    }

    // 3. Read the file content
    log::info!("Reading content from {}", PRICES_FILE_PATH);
    let content = fs::read_to_string(PRICES_FILE_PATH)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Trim whitespace to avoid accidental newlines taking up buffer space
    let trimmed = content.trim().to_string();

    if trimmed.is_empty() {
        return Err("Price file was empty".into());
    }

    Ok(trimmed)
}

/// Converts the raw string into a byte buffer for the keyboard
/// The buffer must be exactly PACKET_SIZE (32 bytes).
fn convert_to_buffer(text: String) -> Vec<u8> {
    // Convert string to bytes
    let mut buf = text.into_bytes();

    // If it's too long, truncate it
    if buf.len() > PACKET_SIZE {
        log::warn!("Output content exceeds {} bytes, truncating.", PACKET_SIZE);
        buf.truncate(PACKET_SIZE);
    }

    // If it's too short, pad with 0 (null bytes)
    if buf.len() < PACKET_SIZE {
        buf.resize(PACKET_SIZE, 0);
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
async fn send_to_keyboard(text: String) -> Result<(), AppError> {
    log::info!("Sending to usb keyboard");

    let api = HidApi::new()?;
    let device = find_elora_device(&api);

    if device.is_none() {
        return Err("Device disconnected".into());
    }

    let device = device.unwrap().open_device(&api);
    let buf = convert_to_buffer(text);

    // Write to device
    device?.write(&buf)?;

    // Debug logging to see what we actually sent
    let debug_str: String = buf
        .iter()
        .map(|&b| {
            if (32..=126).contains(&b) {
                b as char
            } else {
                '.' // Represent null/control bytes as dots
            }
        })
        .collect();
    log::debug!("Buffer sent to HID device: {}", debug_str);

    Ok(())
}

/// Main worker which fetches stuff and sends it to keyboard
async fn run() -> Result<(), AppError> {
    // 1. Execute command and read file
    match fetch_prices_from_local_command().await {
        Ok(content) => {
            // 2. Send to keyboard
            send_to_keyboard(content).await?;
            Ok(())
        }
        Err(e) => {
            // We return error here so main loop can log it, but we don't crash
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    println!(
        r"
 _____ _                    _   _ ___ ____  
| ____| | ___  _ __ __ _   | | | |_ _|  _ \ 
|  _| | |/ _ \| '__/ _` |  | |_| || || | | |
| |___| | (_) | | | (_| |  |  _  || || |_| |
|_____|_|\___/|_|  \__,_|  |_| |_|___|____/ 
"
    );

    let api_check = HidApi::new();
    if let Ok(api) = api_check {
        if find_elora_device(&api).is_none() {
            log::warn!("Elora keyboard not detected at startup. Will retry in loop.");
        }
    }

    loop {
        // Run logic
        if let Err(e) = run().await {
            log::error!("Run failed (will retry next cycle): {}", e);
        }

        // Recalculate refresh rate based on market hours
        let refresh_rate = get_refresh_rate();

        let mut interval = tokio::time::interval(refresh_rate);
        interval.reset(); // Align interval
        interval.tick().await; // Wait for next tick
    }
}

#[test]
fn testing_conversion_to_buffer_truncation() {
    let input = "This string is way too long for the 32 byte limit".to_string();
    let buf = convert_to_buffer(input);
    assert_eq!(buf.len(), PACKET_SIZE);

    // Check if it truncated correctly
    let string_part = std::str::from_utf8(&buf).unwrap();
    assert_eq!(string_part, "This string is way too long for ");
}

#[test]
fn testing_conversion_to_buffer_padding() {
    let input = "Short".to_string();
    let buf = convert_to_buffer(input);
    assert_eq!(buf.len(), PACKET_SIZE);

    // Check content
    assert_eq!(buf[0], b'S');
    assert_eq!(buf[4], b't');
    assert_eq!(buf[5], 0); // Padding starts
}
