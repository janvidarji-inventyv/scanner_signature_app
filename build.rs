fn main() {
    //println!("cargo:rustc-link-search=native=/home/janvi/Android/Sdk/ndk/30.0.14904198/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/30/");
    #[cfg(target_os = "android")]
    {
        println!("cargo:rustc-link-lib=camera2ndk");
        println!("cargo:rustc-link-lib=mediandk");
    }
}