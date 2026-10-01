//! Converts a rendered receipt PNG into an ESC/POS job.
//!
//!   cargo run -p escpos --example png2escpos -- receipt-80mm.png out.bin [80|58] [--preview 1bit.png]
//!   # Windows, prints silently:
//!   cargo run -p escpos --example png2escpos -- receipt-80mm.png out.bin 80 --print "POS-80"
//!   cargo run -p escpos --example png2escpos -- --list
use escpos::{receipt_job, Bitmap, Dither, Paper};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") {
        #[cfg(windows)]
        {
            println!("default: {:?}", escpos::spooler::default_printer());
            for p in escpos::spooler::list_printers().unwrap() {
                println!("  {p}");
            }
        }
        #[cfg(not(windows))]
        eprintln!("--list is Windows-only");
        return;
    }
    let (input, output) = (&args[0], &args[1]);
    let paper = match args.get(2).map(String::as_str) {
        Some("58") => Paper::Mm58,
        _ => Paper::Mm80,
    };
    let png = std::fs::read(input).expect("read png");
    let img = Bitmap::from_png(&png, Dither::Threshold(160)).expect("decode").trim_bottom();
    let job = receipt_job(&img, paper, false).expect("encode");
    std::fs::write(output, &job).expect("write");
    println!("{}x{} dots -> {} bytes ESC/POS", img.width, img.height, job.len());

    if let Some(i) = args.iter().position(|a| a == "--preview") {
        // 1-bit preview of exactly what the print head will burn.
        let f = std::fs::File::create(&args[i + 1]).expect("create preview");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(f), img.width, img.height);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::One);
        let inverted: Vec<u8> = img.data.iter().map(|b| !b).collect();
        enc.write_header().unwrap().write_image_data(&inverted).unwrap();
    }

    if let Some(i) = args.iter().position(|a| a == "--print") {
        #[cfg(windows)]
        escpos::spooler::print_raw(&args[i + 1], "Artaveo receipt", &job).expect("print");
        #[cfg(not(windows))]
        {
            let _ = i;
            eprintln!("--print is Windows-only");
        }
    }
}
