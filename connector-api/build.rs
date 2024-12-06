fn main() {
    if std::env::var("DEBUG").is_ok() {
        println!("cargo:rustc-cfg=disable_auth");
    }
}