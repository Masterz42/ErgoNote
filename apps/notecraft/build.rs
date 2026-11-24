//! Windows only: gives the .exe its file icon and its version information, so Explorer, the task
//! bar entry and the Properties dialog show NoteCraft. The version is the workspace version.
//! On any other system this does nothing, and the `winresource` crate is not even built.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/notecraft.ico");
    println!("cargo:rerun-if-env-changed=RC_PATH");
    let mut res = winresource::WindowsResource::new();
    #[cfg(not(windows))]
    if std::path::Path::new("/usr/lib/llvm-18/bin/llvm-windres").exists() {
        res.set_windres_path("/usr/lib/llvm-18/bin/llvm-windres");
        res.set_ar_path("/usr/lib/llvm-18/bin/llvm-ar");
    }
    res.set_icon("../../assets/app-icon/notecraft.ico");
    res.set("ProductName", "ErgoNote");
    res.set("FileDescription", "ErgoNote - Cahier numerique scolaire adapte TND");
    res.set("LegalCopyright", "Freeware - All rights reserved (c) ErgoNote");
    if let Err(e) = res.compile() {
        #[cfg(not(windows))]
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
            && std::path::Path::new("/usr/lib/llvm-18/bin/llvm-rc").exists()
            && let (Ok(out_dir), Ok(manifest_dir)) = (std::env::var("OUT_DIR"), std::env::var("CARGO_MANIFEST_DIR"))
        {
            let rc_path = std::path::Path::new(&out_dir).join("main_res.rc");
            let res_path = std::path::Path::new(&out_dir).join("main_res.res");
            if res.write_resource_file(&rc_path).is_ok()
                && let Ok(status) = std::process::Command::new("/usr/lib/llvm-18/bin/llvm-rc")
                    .arg(format!("/I{manifest_dir}"))
                    .arg(format!("/fo{}", res_path.display()))
                    .args(["/C", "65001", "--"])
                    .arg(&rc_path)
                    .status()
                && status.success()
            {
                println!("cargo:rustc-link-arg={}", res_path.display());
                return;
            }
        }
        println!("cargo:warning=could not embed the icon: {e}");
    }
}
