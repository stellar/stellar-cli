fn main() {
    set_protocol_features();
}

fn set_protocol_features() {
    let version = env!("CARGO_PKG_VERSION");
    let major_version: u32 = version
        .split('.')
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    if major_version >= 23 {
        println!("cargo:rustc-cfg=feature=\"version_gte_23\"");
    }

    if major_version < 23 && std::env::var("CARGO_FEATURE_VERSION_GTE_23").is_err() {
        println!("cargo:rustc-cfg=feature=\"version_lt_23\"");
    }
}
