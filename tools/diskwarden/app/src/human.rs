//! Render bytes and ages the way a person reads them.

/// Bytes → "92.4 GB". Base-1024, matching what `du -h` shows you.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

/// Seconds → "5 min" / "1 hr" / "4 hr". Used for the scan-interval menu.
pub fn interval(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} sec"),
        60 => "1 min".into(),
        61..=3599 => format!("{} min", secs / 60),
        3600 => "1 hr".into(),
        _ => format!("{} hr", secs / 3600),
    }
}

/// Days → "today" / "3d" / "2mo".
pub fn age(days: u64) -> String {
    match days {
        0 => "today".into(),
        1 => "1d".into(),
        2..=59 => format!("{days}d"),
        _ => format!("{}mo", days / 30),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_to_readable_units() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1024), "1.0 KB");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bytes(92 * 1024 * 1024 * 1024), "92.0 GB");
    }

    #[test]
    fn stops_at_terabytes_rather_than_running_off_the_unit_table() {
        assert_eq!(bytes(u64::MAX), "16777216.0 TB");
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age(0), "today");
        assert_eq!(age(1), "1d");
        assert_eq!(age(45), "45d");
        assert_eq!(age(90), "3mo");
    }

    #[test]
    fn intervals_read_naturally() {
        assert_eq!(interval(60), "1 min");
        assert_eq!(interval(300), "5 min");
        assert_eq!(interval(900), "15 min");
        assert_eq!(interval(3600), "1 hr");
        assert_eq!(interval(14400), "4 hr");
    }
}
