extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn add_binding(header_path: &str) {
    println!("cargo:rerun-if-changed={}", header_path);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let binding_path = out_dir.join(header_path.replace(".h", ".rs"));
    std::fs::create_dir_all(binding_path.parent().unwrap())
        .expect(&format!("Cannot create directory for {}", binding_path.to_string_lossy()));

    bindgen::Builder::default()
        .header(header_path)
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect(&format!("Unable to generate binding {}", header_path))
        .write_to_file(binding_path)
        .expect("Cannot write binding");
}

fn main() {
    add_binding("src/driver/scsi/bindings.h");

    add_binding("src/vncclient/bindings.h");
    println!("cargo:rustc-link-lib=vncclient");

}
