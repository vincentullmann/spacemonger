//! Size / date formatting (port of FormatService).

use chrono::{Local, TimeZone};
use humansize::{FormatSizeOptions, BINARY, DECIMAL};
use num_format::{Locale, ToFormattedString};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::RwLock;

/// Default `strftime` pattern for dates.
pub const DEFAULT_DATE_FORMAT: &str = "%d %b %Y   %-H:%M:%S";

/// How file sizes are written.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SizeFormat {
    /// "1,234,567 bytes".
    #[default]
    Bytes,

    /// Powers of 1024: "1.2 MiB".
    Binary,

    /// Powers of 1000: "1.2 MB".
    Decimal,

    /// "38.8 KiB (39,718 bytes)".
    BinaryAndBytes,

    /// "39.7 kB (39,718 bytes)".
    DecimalAndBytes,
}

/// App-wide formatting choices (set from the settings).
#[derive(Clone, Debug, PartialEq)]
pub struct FormatOptions {
    pub size: SizeFormat,
    /// `strftime` pattern; an invalid one falls back to [`DEFAULT_DATE_FORMAT`].
    pub date_format: String,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            size: SizeFormat::Bytes,
            date_format: DEFAULT_DATE_FORMAT.to_string(),
        }
    }
}

static OPTIONS: RwLock<Option<FormatOptions>> = RwLock::new(None);

pub fn set_options(o: FormatOptions) {
    if let Ok(mut g) = OPTIONS.write() {
        *g = Some(o);
    }
}

fn with_options<T>(f: impl FnOnce(&FormatOptions) -> T) -> T {
    match OPTIONS.read().ok().as_deref().and_then(Option::as_ref) {
        Some(o) => f(o),
        None => f(&FormatOptions::default()),
    }
}

/// One decimal, in powers of 1024 ("KiB", "MiB", ...) or 1000 ("kB", "MB", ...).
fn size_format() -> FormatSizeOptions {
    let decimal =
        with_options(|o| matches!(o.size, SizeFormat::Decimal | SizeFormat::DecimalAndBytes));
    let base = if decimal { DECIMAL } else { BINARY };
    base.decimal_places(1).decimal_zeroes(1)
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

/// A file's size as the options say: "1,234,567 bytes", "1.2 MiB", "1.2 MB", or a unit
/// size followed by the bytes.
pub fn file_size(size: u64) -> String {
    match with_options(|o| o.size) {
        SizeFormat::Bytes => bytes(size),
        SizeFormat::Binary | SizeFormat::Decimal => size_string(size, 0, false),
        SizeFormat::BinaryAndBytes | SizeFormat::DecimalAndBytes => {
            format!("{} ({})", size_string(size, 0, false), bytes(size))
        }
    }
}

/// "1,234,567 bytes"
pub fn bytes(size: u64) -> String {
    format!("{} bytes", size.to_formatted_string(&Locale::en))
}

/// "05 Mar 2024   14:07:09" in local time (or the date format from the options).
pub fn date(secs: i64) -> String {
    with_options(|o| date_with(secs, &o.date_format))
}

/// `secs` in local time with a `strftime` pattern; falls back to the default for a bad one.
pub fn date_with(secs: i64, pattern: &str) -> String {
    let dt = Local
        .timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(|| Local.timestamp_opt(0, 0).unwrap());
    let mut s = String::new();
    if write!(s, "{}", dt.format(pattern)).is_err() {
        s.clear();
        let _ = write!(s, "{}", dt.format(DEFAULT_DATE_FORMAT));
    }
    s
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
        assert_eq!(bytes(0), "0 bytes");
        assert_eq!(bytes(999), "999 bytes");
        assert_eq!(bytes(1234567), "1,234,567 bytes");
    }

    #[test]
    fn dates() {
        let secs = Local
            .with_ymd_and_hms(2024, 3, 5, 4, 7, 9)
            .unwrap()
            .timestamp();
        assert_eq!(date(secs), "05 Mar 2024   4:07:09");
        let secs = Local
            .with_ymd_and_hms(2024, 12, 25, 14, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(date(secs), "25 Dec 2024   14:00:00");
        assert_eq!(date_with(secs, "%Y-%m-%d"), "2024-12-25");
        assert_eq!(date_with(secs, "%Q bad"), "25 Dec 2024   14:00:00");
    }
}
