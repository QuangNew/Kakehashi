fn main() {
    #[cfg(target_arch = "x86_64")]
    let avx2 = std::is_x86_feature_detected!("avx2")
        && std::is_x86_feature_detected!("fma")
        && std::is_x86_feature_detected!("f16c")
        && std::is_x86_feature_detected!("bmi2");
    #[cfg(not(target_arch = "x86_64"))]
    let avx2 = false;
    println!("{}", if avx2 { "avx2" } else { "portable" });
}
