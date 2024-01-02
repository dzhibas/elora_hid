use hidapi::HidApi;

/// splitkb.com vendor and Elora product
const VENDOR_ID: u16 = 0x8d1d;
const PRODUCT_ID: u16 = 0x9d9d;

fn main() {
    println!("Printing hid devices: ");
    match HidApi::new() {
        Ok(api) => {
            for dev in api.device_list() {
                if dev.vendor_id() == VENDOR_ID && dev.product_id() == PRODUCT_ID {
                    println!(
                        "{:04x}:{:04x} {:?} {:?}",
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
