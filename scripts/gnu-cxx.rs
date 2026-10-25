//! whisper-rs-sys 0.15 adds MSVC's /utf-8 on all Windows targets.
//! GCC already reads UTF-8; remove that one unsupported flag for MinGW builds.
fn main() {
    let compiler = std::env::var_os("TVNJP_CXX_PATH").expect("Set TVNJP_CXX_PATH to g++.exe");
    let arguments = std::env::args_os().skip(1).filter(|arg| arg != "/utf-8");
    let status = std::process::Command::new(compiler).args(arguments).status().expect("Cannot run C++ compiler");
    std::process::exit(status.code().unwrap_or(1));
}
