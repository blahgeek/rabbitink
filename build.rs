extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindgen::Builder::default()
        .header("src/driver/scsi/wrapper.h")
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate scsi wrapper bindings")
        .write_to_file(out_path.join("scsi_bindings.rs"))
        .expect("Couldn't write bindings!");
}
