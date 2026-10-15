// The DDS encoder (intel_tex_2) contains C++ and does not ask for the C++
// runtime itself, so a binary without other C++ in it fails to link.
fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target.as_str() {
        "macos" => println!("cargo:rustc-link-lib=c++"),
        "windows" => {}
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }
}
