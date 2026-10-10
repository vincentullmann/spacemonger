//! Size / date formatting (port of FormatService).

use chrono::{Local, TimeZone};
use humansize::{FormatSizeOptions, BINARY};
use num_format::{Locale, ToFormattedString};

/// Powers of 1024 ("KiB", "MiB", ...) with one decimal.
fn size_format() -> FormatSizeOptions {
    BINARY.decimal_places(1).decimal_zeroes(1)
}

/// "12.3 GiB" (rounded), or a percentage of `total` ("45.6%") when `percent` is set.
pub fn size_string(size: u64, total: u64, percent: bool) -> String {
    if percent {
        let total = if total == 0 { u64::MAX } else { total };
        let v = (size as u128 * 1000 / total as u128) as u64;
        return format!("{}.{}%", v / 10, v % 10);
    }
    humansize::format_size(size, size_format())
}

/// "1,234,567 bytes"
pub fn file_size(size: u64) -> String {
    format!("{} bytes", size.to_formatted_string(&Locale::en))
}

/// "05 Mar 2024   14:07:09" in local time.
pub fn date(secs: i64) -> String {
    let dt = Local
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(0, 0).unwrap());
    dt.format("%d %b %Y   %-H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size_string(500, 0, false), "500.0 B");
        assert_eq!(size_string(1536, 0, false), "1.5 KiB");
        assert_eq!(size_string(1023 * 1024 + 1000, 0, false), "1024.0 KiB");
        assert_eq!(size_string(3 * 1024 * 1024 * 1024, 0, false), "3.0 GiB");
        assert_eq!(size_string(5 << 40, 0, false), "5.0 TiB");
        assert_eq!(size_string(456, 1000, true), "45.6%");
        assert_eq!(file_size(0), "0 bytes");
        assert_eq!(file_size(999), "999 bytes");
        assert_eq!(file_size(1234567), "1,234,567 bytes");
    }

    #[test]
    fn dates() {
        let secs = Local.with_ymd_and_hms(2024, 3, 5, 4, 7, 9).unwrap().timestamp();
        assert_eq!(date(secs), "05 Mar 2024   4:07:09");
        let secs = Local.with_ymd_and_hms(2024, 12, 25, 14, 0, 0).unwrap().timestamp();
        assert_eq!(date(secs), "25 Dec 2024   14:00:00");
    }
}
