//! `available_ram_bytes` — how much memory this machine can hand out *right now*, read straight
//! from `/proc/meminfo`'s `MemAvailable` (boot-memory-guard scope, decision 5).
//!
//! Deliberately dependency-free: the store crate already probes the OS by reading proc files
//! (`wait_for_quiesce`'s `/proc/self/fd` scan) and refuses to grow a `sysinfo`/`libc` dependency
//! for one integer. `MemAvailable` — not `MemFree` — is the kernel's own estimate of what a new
//! allocation can get without swapping, which is exactly the question the boot guards ask.
//!
//! **Absent ⇒ `None` ⇒ both guards fail open.** Non-Linux, a container with an odd `/proc` mount,
//! or a kernel too old for `MemAvailable` all land here, and a heuristic that can brick a valid
//! boot on a machine it cannot measure is worse than the bug it prevents.

/// Bytes of memory available for a new allocation, or `None` when this machine cannot be measured.
pub fn available_ram_bytes() -> Option<u64> {
    parse_mem_available(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

/// Pull `MemAvailable` (reported in kB) out of `/proc/meminfo` text. Split from the read so the
/// parse is testable without a filesystem.
pub(crate) fn parse_mem_available(meminfo: &str) -> Option<u64> {
    let line = meminfo
        .lines()
        .find(|l| l.starts_with("MemAvailable:"))?
        .strip_prefix("MemAvailable:")?;
    let kb: u64 = line.split_whitespace().next()?.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

#[cfg(test)]
mod tests {
    use super::parse_mem_available;

    #[test]
    fn parses_mem_available_as_bytes() {
        let sample = "MemTotal:         982012 kB\nMemFree:          102400 kB\nMemAvailable:     802048 kB\nBuffers:           1024 kB\n";
        assert_eq!(parse_mem_available(sample), Some(802_048 * 1024));
    }

    #[test]
    fn absent_or_malformed_is_none() {
        assert_eq!(parse_mem_available("MemTotal: 982012 kB\n"), None);
        assert_eq!(parse_mem_available("MemAvailable:  lots kB\n"), None);
        assert_eq!(parse_mem_available(""), None);
    }

    #[test]
    fn a_real_linux_box_reports_something() {
        // Not an assertion about the number — only that the real read path works where /proc is
        // mounted, and degrades to None where it is not. Never a mock: this reads the real file.
        if std::path::Path::new("/proc/meminfo").exists() {
            assert!(super::available_ram_bytes().unwrap_or(0) > 0);
        }
    }
}
