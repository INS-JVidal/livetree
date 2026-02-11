use std::fs;
use std::path::Path;

fn main() {
    let path = Path::new("BUILD_NUMBER");
    let current: u64 = fs::read_to_string(path)
        .unwrap_or_else(|_| "0".to_string())
        .trim()
        .parse()
        .unwrap_or(0);

    let next = current + 1;
    fs::write(path, format!("{}\n", next)).expect("failed to write BUILD_NUMBER");

    println!("cargo:rustc-env=BUILD_NUMBER={}", next);
    // No rerun-if-changed for BUILD_NUMBER: this script modifies it on every
    // build, so cargo always detects a change and reruns — which is the desired
    // behaviour (every compilation gets a fresh build number).
    println!("cargo:rerun-if-changed=build.rs");
}
