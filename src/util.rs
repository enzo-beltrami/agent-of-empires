//! Small crate-internal utilities shared across modules.

use std::time::{SystemTime, UNIX_EPOCH};

/// `path` with a leading `home` replaced by `~`, only when `path` is `home` or
/// lies under it; a sibling that merely shares a string prefix is unchanged.
pub(crate) fn collapse_home(path: &str, home: &str) -> String {
    let home = match home.trim_end_matches('/') {
        "" => home,
        trimmed => trimmed,
    };
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

/// [`collapse_home`] against the user's home directory, for display.
pub(crate) fn collapse_tilde(path: &str) -> String {
    match dirs::home_dir() {
        Some(home) => collapse_home(path, &home.to_string_lossy()),
        None => path.to_string(),
    }
}

/// Current Unix time in whole seconds, saturating to 0 if the clock is before
/// the epoch (which should never happen on a sane system).
pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whole milliseconds between `t` and the Unix epoch, saturating to 0 if `t`
/// predates the epoch.
pub(crate) fn system_time_to_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Current Unix time in whole milliseconds. Wall-clock (not a per-process
/// monotonic), so values are comparable across processes; saturating to 0 if
/// the clock is before the epoch.
pub(crate) fn now_ms() -> u64 {
    system_time_to_ms(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn collapse_home_requires_a_separator_after_home() {
        for (path, expect) in [
            ("/home/u", "~"),
            ("/home/u/projects/app", "~/projects/app"),
            ("/home/u/projects/", "~/projects/"),
            ("/home/uextra/not/home", "/home/uextra/not/home"),
            ("/tmp/elsewhere", "/tmp/elsewhere"),
            ("relative/path", "relative/path"),
        ] {
            assert_eq!(collapse_home(path, "/home/u"), expect, "{path}");
            assert_eq!(
                collapse_home(path, "/home/u/"),
                expect,
                "{path} (trailing /)"
            );
        }
    }

    #[test]
    fn system_time_to_ms_converts_and_saturates() {
        assert_eq!(system_time_to_ms(UNIX_EPOCH), 0);
        assert_eq!(
            system_time_to_ms(UNIX_EPOCH + Duration::from_millis(1_500)),
            1_500
        );
        assert_eq!(system_time_to_ms(UNIX_EPOCH - Duration::from_secs(1)), 0);
        let t = SystemTime::now();
        let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(system_time_to_ms(t) / 1_000, secs);
    }
}
