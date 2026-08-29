fn main() {
    println!("cargo:rerun-if-changed=../src/skin");
    println!("cargo:rerun-if-changed=../src/assets");
    println!("cargo:rerun-if-changed=../src/index.html");
    println!("cargo:rerun-if-changed=../src/styles.css");
    println!("cargo:rerun-if-changed=../src/app.js");
    tauri_build::build()
}
