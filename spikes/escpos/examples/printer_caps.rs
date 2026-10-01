//! Windows: which installed printers can take A5/A6 directly and have a manual tray.
//!   cargo run -p escpos --example printer_caps
fn main() {
    #[cfg(windows)]
    for p in escpos::spooler::list_printers().unwrap() {
        match escpos::spooler::capabilities(&p) {
            Ok(c) => println!(
                "{p}: small-paper profiles {:?}, manual feed {:?}, {} sizes, bins {:?}",
                c.small_paper_profiles(),
                c.manual_feed_bin().map(|b| &b.name),
                c.papers.len(),
                c.bins.iter().map(|b| &b.name).collect::<Vec<_>>()
            ),
            Err(e) => println!("{p}: {e}"),
        }
    }
    #[cfg(not(windows))]
    eprintln!("printer_caps is Windows-only");
}
