use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use flate2::Compression;
use flate2::write::ZlibEncoder;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let payload_src = manifest_dir.join("../app/target/release/uwidgets-rs.exe");
    let icon_path = manifest_dir.join("../../assets/uWidgets.ico");
    println!("cargo:rerun-if-changed={}", payload_src.display());
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let compressed_dst = out_dir.join("uwidgets_payload.zlib");

    let raw_bytes = fs::read(&payload_src).unwrap_or_else(|e| {
        panic!(
            "Failed to read release app binary at {}: {e}. Build `rust/app` with `cargo build --release` first.",
            payload_src.display()
        )
    });
    let mut enc = ZlibEncoder::new(Vec::with_capacity(raw_bytes.len() / 2), Compression::best());
    enc.write_all(&raw_bytes).expect("compress payload");
    let compressed = enc.finish().expect("finish zlib stream");
    fs::write(&compressed_dst, &compressed).expect("write compressed payload");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/uWidgets.ico");
        res.set("ProductName", "uWidgets (Rust Edition) Setup");
        res.set(
            "FileDescription",
            "uWidgets (Rust Edition) v1.0.0 Windows Setup Installer",
        );
        res.set("CompanyName", "uWidgets-rs");
        res.set("OriginalFilename", "uWidgets-Rust-v1.0.0-Setup.exe");
        res.set("FileVersion", "1.0.0.0");
        res.set("ProductVersion", "1.0.0");
        if let Err(err) = res.compile() {
            println!("cargo:warning=winres setup icon embedding failed: {err}");
        }
    }
}
