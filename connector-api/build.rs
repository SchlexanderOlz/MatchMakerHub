fn main() {
    if option_env!("DEBUG").is_some_and(|x| x == "1") {
        println!("cargo:rustc-cfg=disable_auth");
        println!("cargo:rustc-cfg=disable_elo");
    }
}