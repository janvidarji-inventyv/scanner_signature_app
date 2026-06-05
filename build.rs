use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    if target.contains("android") {
        println!("cargo:rustc-link-lib=camera2ndk");
        println!("cargo:rustc-link-lib=mediandk");
        configure_android_launcher_icon();
    }
}

fn configure_android_launcher_icon() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let root = PathBuf::from(manifest_dir);

    let source_icon = root.join("src/assets/icon_sign_pad.webp");
    let res_root = root.join("android/app/src/main/res");

    println!("cargo:rerun-if-changed={}", source_icon.display());

    if !source_icon.exists() {
        println!(
            "cargo:warning=Android launcher icon source not found: {}",
            source_icon.display()
        );
        return;
    }

    let mipmap_dirs = [
        "mipmap-mdpi",
        "mipmap-hdpi",
        "mipmap-xhdpi",
        "mipmap-xxhdpi",
        "mipmap-xxxhdpi",
    ];

    for dir in mipmap_dirs {
        let out_dir = res_root.join(dir);
        if let Err(e) = fs::create_dir_all(&out_dir) {
            println!("cargo:warning=Failed to create {}: {e}", out_dir.display());
            continue;
        }

        copy_icon(&source_icon, &out_dir.join("ic_launcher.webp"));
        copy_icon(&source_icon, &out_dir.join("ic_launcher_round.webp"));
        copy_icon(&source_icon, &out_dir.join("ic_launcher_foreground.webp"));
    }

    let anydpi = res_root.join("mipmap-anydpi-v26");
    if let Err(e) = fs::create_dir_all(&anydpi) {
        println!("cargo:warning=Failed to create {}: {e}", anydpi.display());
        return;
    }

    let adaptive_icon_xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    \
<background android:drawable=\"@android:color/white\" />\n    \
<foreground>\n        \
<inset android:drawable=\"@mipmap/ic_launcher_foreground\" android:inset=\"20%\" />\n    \
</foreground>\n    \
<monochrome>\n        \
<inset android:drawable=\"@mipmap/ic_launcher_foreground\" android:inset=\"20%\" />\n    \
</monochrome>\n\
</adaptive-icon>\n";

    if let Err(e) = fs::write(anydpi.join("ic_launcher.xml"), adaptive_icon_xml) {
        println!("cargo:warning=Failed to write adaptive icon xml: {e}");
    }
    if let Err(e) = fs::write(anydpi.join("ic_launcher_round.xml"), adaptive_icon_xml) {
        println!("cargo:warning=Failed to write adaptive round icon xml: {e}");
    }
}

fn copy_icon(from: &Path, to: &Path) {
    if let Err(e) = fs::copy(from, to) {
        println!(
            "cargo:warning=Failed to copy icon {} -> {}: {e}",
            from.display(),
            to.display()
        );
    }
}