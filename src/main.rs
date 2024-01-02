use std::time::Duration;

use hidapi::HidApi;

/// splitkb.com vendor id
const VENDOR_ID: u16 = 0x8d1d;
/// Elora product id
const PRODUCT_ID: u16 = 0x9d9d;

/// How often to refetch new data from dependency services
const REFRESH_RATE_SECS:u16 = 60;

async fn fetch_stock_tickers() {
    println!("Run of stock tickers function");
}

async fn run() {
    fetch_stock_tickers().await;

    println!("Printing hid devices: ");
    match HidApi::new() {
        Ok(api) => {
            for dev in api.device_list() {
                if dev.vendor_id() == VENDOR_ID && dev.product_id() == PRODUCT_ID {
                    println!(
                        "{:03x}:{:04x} {:?} {:?}",
                        dev.vendor_id(),
                        dev.product_id(),
                        dev.manufacturer_string(),
                        dev.product_string()
                    );
                    dbg!(dev);
                }
            }
        }
        Err(_) => eprintln!("Error happened"),
    }
}

#[tokio::main]
async fn main() {
    let mut interval = tokio::time::interval(Duration::from_secs(REFRESH_RATE_SECS.into()));
    loop {
        interval.tick().await;
        run().await;
    }
}
