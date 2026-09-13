fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/dialkey.ico");
        println!("cargo:rerun-if-changed=assets/dialkey.manifest");
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/dialkey.ico");
        res.set_manifest_file("assets/dialkey.manifest");
        res.set("ProductName", "DialKey");
        res.set("FileDescription", "DialKey — software macropad");
        res.set("LegalCopyright", "MIT");
        res.compile().unwrap();
    }

    // Place sample assets next to the built exe so slots.example paths work
    // under `cargo run` (resolved against the executable directory).
    println!("cargo:rerun-if-changed=samples/hello.bat");
    println!("cargo:rerun-if-changed=README.md");
    if let Some(out_dir) = std::env::var_os("OUT_DIR") {
        // OUT_DIR = target/<profile>/build/<crate>/out → climb to profile dir
        let profile_dir = std::path::Path::new(&out_dir)
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf());
        if let Some(profile_dir) = profile_dir {
            let _ = std::fs::create_dir_all(profile_dir.join("samples"));
            let _ = std::fs::copy("samples/hello.bat", profile_dir.join("samples/hello.bat"));
            let _ = std::fs::copy("README.md", profile_dir.join("README.md"));
        }
    }
}
