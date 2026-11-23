fn main() {
    println!("cargo:rerun-if-changed=../../assets/uWidgets.ico");
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/uWidgets.ico");
        res.set("ProductName", "uWidgets (Rust Edition)");
        res.set(
            "FileDescription",
            "Ultra-lightweight Liquid Glass Desktop Widgets for Windows",
        );
        res.set("CompanyName", "uWidgets-rs");
        res.set("OriginalFilename", "uwidgets-rs.exe");
        if let Err(err) = res.compile() {
            println!("cargo:warning=winres icon embedding failed: {err}");
        }
    }
}
