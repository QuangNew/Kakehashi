//! Stored transcript representation and legacy timestamp compatibility.
#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    pub id: String,
    pub text: String,
    pub speaker: String,
    pub timestamp_ms: u64,
}
pub fn speaker_label(speaker: &str) -> &str {
    match speaker {
        "User" => "You",
        other => other,
    }
}

pub fn relative_ms(timestamp_ms: i64, start_ms: i64) -> u64 {
    let ts = timestamp_ms.max(0);
    if start_ms > 0 && ts >= start_ms {
        (ts - start_ms) as u64
    } else {
        ts as u64
    }
}

pub fn fallback_segment_id(timestamp_ms: i64, speaker: &str, text: &str) -> String {
    use std::hash::{Hash, Hasher};
    // `DefaultHasher::new()` uses fixed keys: the same segment always hashes the same.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (timestamp_ms, speaker, text).hash(&mut hasher);
    format!("seg-{:016x}", hasher.finish())
}
