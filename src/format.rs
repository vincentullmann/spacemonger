//! Size / date formatting (port of FormatService).

use chrono::{Datelike, Local, TimeZone, Timelike};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// "12.3 GB", or a percentage of `total` ("45.6%") when `percent` is set.
pub fn size_string(size: u64, total: u64, percent: bool) -> String {
    if percent {
        let total = if total == 0 { u64::MAX } else { total };
        let v = (size as u128 * 1000 / total as u128) as u64;
        return format!("{}.{}%", v / 10, v % 10);
    }
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unit, div, label) = if size < K {
        (size, 1, "bytes")
    } else if size < M {
        (size, K, "KB")
    } else if size < G {
        (size, M, "MB")
    } else {
        (size, G, "GB")
    };
    let full = unit / div;
    let frac = if div == 1 { 0 } else { 10 * (unit % div) / div };
    format!("{full}.{frac} {label}")
}

/// "1,234,567 bytes"
pub fn file_size(size: u64) -> String {
    let digits = size.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 6);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out.push_str(" bytes");
    out
}

/// "05 Mar 2024   14:07:09" in local time.
pub fn date(secs: i64) -> String {
    let dt = Local
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(0, 0).unwrap());
    format!(
        "{:02} {} {:04}   {}:{:02}:{:02}",
        dt.day(),
        MONTHS[dt.month0() as usize],
        dt.year(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size_string(500, 0, false), "500.0 bytes");
        assert_eq!(size_string(1536, 0, false), "1.5 KB");
        assert_eq!(size_string(3 * 1024 * 1024 * 1024, 0, false), "3.0 GB");
        assert_eq!(size_string(456, 1000, true), "45.6%");
        assert_eq!(file_size(0), "0 bytes");
        assert_eq!(file_size(999), "999 bytes");
        assert_eq!(file_size(1234567), "1,234,567 bytes");
    }
}
