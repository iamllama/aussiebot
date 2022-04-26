// build.rs

use std::env;
use std::fs;
use std::path::Path;
use std::time::*;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("timestamp.rs");

    fs::write(
        &dest_path,
        format!(
            "//Returns the build timestamp, for versioning purposes
pub const fn _build_timestamp() -> &'static str {{
   \"{:?}\"
}}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        ),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=src");
}
