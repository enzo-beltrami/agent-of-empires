//! Terminal utilities and helpers

/// Returns the controlling terminal's size as (width, height), or None if unavailable.
#[cfg(unix)]
pub fn get_size() -> Option<(u16, u16)> {
    [libc::STDOUT_FILENO, libc::STDERR_FILENO, libc::STDIN_FILENO]
        .into_iter()
        .find_map(|fd| {
            // SAFETY: TIOCGWINSZ only writes into the zeroed `winsize` we own.
            let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
            let ok = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0;
            (ok && ws.ws_col > 0 && ws.ws_row > 0).then_some((ws.ws_col, ws.ws_row))
        })
}

/// Returns None: terminal size detection is unix-only.
#[cfg(not(unix))]
pub fn get_size() -> Option<(u16, u16)> {
    None
}
