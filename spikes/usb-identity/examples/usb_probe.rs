//! Windows: lists USB volumes with their identity, then watches for plug/unplug.
//!   cargo run -p usb-identity --example usb_probe [-- --once]
fn main() {
    #[cfg(windows)]
    {
        let print = || {
            for v in usb_identity::windows::usb_volumes() {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            }
        };
        print();
        if std::env::args().any(|a| a == "--once") {
            return;
        }
        let mut last = usb_identity::windows::drive_mask();
        println!("watching for USB changes (Ctrl+C to stop)...");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let now = usb_identity::windows::drive_mask();
            if now != last {
                println!("--- drive set changed ---");
                print();
                last = now;
            }
        }
    }
    #[cfg(not(windows))]
    eprintln!("usb_probe is Windows-only");
}
