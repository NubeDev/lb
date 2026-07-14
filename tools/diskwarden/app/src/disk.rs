//! How full is the disk? Drives the icon colour.
//!
//! Shells out to `df` rather than pulling a libc/statvfs binding. This runs once per
//! scan (every 5 min), so the process cost is irrelevant, and `df` is the same number
//! you'd check by hand — no chance of us and you disagreeing about "free".

use std::path::Path;
use std::process::Command;

/// Free-space facts for one filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Free {
    pub free_bytes: u64,
    pub total_bytes: u64,
}

impl Free {
    pub fn free_pct(&self) -> u8 {
        if self.total_bytes == 0 {
            return 100; // unknown → assume healthy rather than cry wolf
        }
        ((self.free_bytes as u128 * 100) / self.total_bytes as u128) as u8
    }
}

/// Ask `df` about the filesystem holding `path`.
pub fn free_for(path: &Path) -> anyhow::Result<Free> {
    // -P = POSIX output (one line per fs, never wrapped); -k = 1K blocks.
    let out = Command::new("df").arg("-Pk").arg(path).output()?;
    anyhow::ensure!(
        out.status.success(),
        "df failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `df -Pk` output. Split out from the command so it's testable without a
/// filesystem of a known size.
fn parse_df(stdout: &str) -> anyhow::Result<Free> {
    let line = stdout
        .lines()
        .nth(1) // skip the header
        .ok_or_else(|| anyhow::anyhow!("df produced no data row"))?;

    // Filesystem 1024-blocks Used Available Capacity Mounted-on
    //
    // Neither end is a safe anchor: a device name can contain spaces, and so can a
    // mount point ("/media/My Big Disk"), so counting from either side shifts. The
    // reliable landmark is the capacity column — the only field ending in '%'. The
    // four numbers sit immediately before it, in fixed order.
    let fields: Vec<&str> = line.split_whitespace().collect();
    let pct_at = fields
        .iter()
        .position(|f| f.ends_with('%'))
        .ok_or_else(|| anyhow::anyhow!("no capacity column in df row: {line:?}"))?;
    anyhow::ensure!(pct_at >= 4, "df row is too short: {line:?}");

    // …[blocks] [used] [avail] [capacity%] …
    let total_k: u64 = fields[pct_at - 3].parse()?;
    let avail_k: u64 = fields[pct_at - 1].parse()?;

    Ok(Free {
        free_bytes: avail_k * 1024,
        total_bytes: total_k * 1024,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real output from this box.
    const REAL: &str = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n\
                        /dev/nvme0n1p2   960380408 471859200 440217600      52% /\n";

    #[test]
    fn parses_real_df_output() {
        let f = parse_df(REAL).unwrap();
        assert_eq!(f.free_bytes, 440217600 * 1024);
        assert_eq!(f.total_bytes, 960380408 * 1024);
        assert_eq!(f.free_pct(), 45);
    }

    /// A mount point with a space in it must not shift the columns. Counting from
    /// the right fails here — the extra words land where the numbers should be.
    #[test]
    fn handles_a_mount_point_with_spaces() {
        let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                   /dev/sdb1 1000 250 750 25% /media/My Big Disk\n";
        let f = parse_df(out).unwrap();
        assert_eq!(f.free_bytes, 750 * 1024);
        assert_eq!(f.free_pct(), 75);
    }

    /// …and neither end is safe: a device name can contain spaces too, which is why
    /// the '%' column is the anchor rather than either edge.
    #[test]
    fn handles_a_device_name_with_spaces() {
        let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                   //server/My Share 1000 250 750 25% /mnt/share\n";
        let f = parse_df(out).unwrap();
        assert_eq!(f.free_bytes, 750 * 1024);
    }

    #[test]
    fn a_full_disk_reads_zero_pct() {
        let out = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                   /dev/sda1 1000 1000 0 100% /\n";
        assert_eq!(parse_df(out).unwrap().free_pct(), 0);
    }

    /// Never divide by zero, and never invent a crisis from a weird fs.
    #[test]
    fn a_zero_sized_fs_reports_healthy_rather_than_panicking() {
        let f = Free {
            free_bytes: 0,
            total_bytes: 0,
        };
        assert_eq!(f.free_pct(), 100);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_df("").is_err());
        assert!(parse_df("header only\n").is_err());
        assert!(parse_df("h\na b c d e f\n").is_err(), "non-numeric columns");
    }

    /// The real thing, on the real machine.
    #[test]
    fn queries_the_real_root_filesystem() {
        let f = free_for(Path::new("/")).unwrap();
        assert!(f.total_bytes > 0, "root fs must have a size");
        assert!(f.free_bytes <= f.total_bytes);
        assert!(f.free_pct() <= 100);
    }
}
