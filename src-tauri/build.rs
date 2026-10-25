fn main() {
    // Fix CRT linking: ort_sys and whisper_rs_sys are compiled with /MD (dynamic CRT)
    // but Rust uses /MT (static CRT). We need to swap static CRT for dynamic CRT.
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        // Remove static CRT, add dynamic CRT to resolve __imp_* symbols
        println!("cargo:rustc-link-arg=/NODEFAULTLIB:libucrt.lib");
        println!("cargo:rustc-link-arg=/DEFAULTLIB:ucrt.lib");
    }

    // Embed one application manifest (Common Controls v6, identical to tauri-build's
    // default) into every link target: the app, unit/integration tests and examples. Without it, test executables that link tauri's dialog code fail to
    // load with STATUS_ENTRYPOINT_NOT_FOUND (comctl32 TaskDialogIndirect needs v6).
    // tauri-build is told not to embed its own copy so the manifests don't collide.
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        // Plain link-arg (not -bins/-tests): cargo only accepts the scoped forms when
        // the package declares such targets, and lib unit tests are not covered by them.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    // MinGW needs the same Common Controls v6 manifest in test/example targets.
    #[cfg(all(target_os = "windows", target_env = "gnu"))]
    {
        // GCC's inline std::call_once uses cross-DLL TLS. In a Rust GNU host,
        // dynamically linking libstdc++ gives its once proxy a different TLS
        // instance and crashes in Vulkan's std::future::wait(). Keep C++'s TLS
        // and proxy together by bundling the matching compiler's static runtime.
        println!("cargo:rerun-if-env-changed=TVNJP_CXX_PATH");
        let compiler = std::env::var_os("TVNJP_CXX_PATH").unwrap_or_else(|| "g++.exe".into());
        let runtime = std::process::Command::new(compiler)
            .arg("-print-file-name=libstdc++.a")
            .output()
            .expect("MinGW C++ compiler is required");
        assert!(runtime.status.success(), "Cannot locate the C++ runtime");
        let runtime = std::path::PathBuf::from(String::from_utf8_lossy(&runtime.stdout).trim());
        assert!(runtime.is_file(), "The MinGW static C++ runtime is missing");
        println!(
            "cargo:rustc-link-search=native={}",
            runtime.parent().unwrap().display()
        );
        // Keep the once/TLS objects even before dependency archives reference
        // them; otherwise the later dynamic stdc++ dependency wins resolution.
        println!("cargo:rustc-link-lib=static:+whole-archive=stdc++");
        println!("cargo:rerun-if-changed={}", runtime.display());
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        let resource = output.join("test-manifest.rc");
        let object = output.join("test-manifest.o");
        std::fs::write(
            &resource,
            format!(
                "1 24 \"{}\"\n",
                manifest.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();
        let status = std::process::Command::new("windres")
            .arg("-i")
            .arg(&resource)
            .arg("-o")
            .arg(&object)
            .args(["-O", "coff"])
            .status()
            .expect("windres is required for MinGW builds");
        assert!(
            status.success(),
            "Could not compile the Common Controls manifest"
        );
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg={}", object.display());
    }
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
