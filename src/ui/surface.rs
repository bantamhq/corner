use ratatui::style::Color;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

/// Terminal-derived color palette.
///
/// Provides a set of gray levels derived from the terminal's background color,
/// useful for creating UI elements that blend naturally with the user's terminal theme.
///
/// For dark terminals: grays get progressively lighter (gray1 = subtle, gray5 = most visible)
/// For light terminals: grays get progressively darker
#[derive(Debug, Clone)]
pub struct Surface {
    /// Whether the terminal has a dark background
    pub is_dark: bool,
    /// The terminal's actual background color
    pub background: Color,
    /// Closest to background (subtle elevation)
    pub gray1: Color,
    /// Slightly further from background
    pub gray2: Color,
    /// Moderate contrast
    pub gray3: Color,
    /// Higher contrast
    pub gray4: Color,
    /// Furthest from background (most visible)
    pub gray5: Color,
    /// Appropriate muted text color for the background luminance
    pub muted_text: Color,
}

impl Default for Surface {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl Surface {
    /// Query terminal background and derive palette.
    ///
    /// **Important**: Call this BEFORE `crossterm::terminal::enable_raw_mode()`.
    ///
    /// Falls back to dark mode defaults if the query fails.
    #[must_use]
    pub fn from_terminal() -> Self {
        query_background()
            .map(|(r, g, b)| Self::from_background(r, g, b))
            .unwrap_or_else(Self::default_dark)
    }

    /// Generate a palette from a known background color.
    #[must_use]
    pub fn from_background(r: u8, g: u8, b: u8) -> Self {
        let lum = luminance(r, g, b);
        let is_dark = lum <= 0.5;

        if is_dark {
            Self::dark_palette(r, g, b, lum)
        } else {
            Self::light_palette(r, g, b, lum)
        }
    }

    /// Fallback palette for dark terminals.
    #[must_use]
    pub fn default_dark() -> Self {
        Self {
            is_dark: true,
            background: Color::Reset,
            gray1: Color::Rgb(25, 25, 25),
            gray2: Color::Rgb(40, 40, 40),
            gray3: Color::Rgb(60, 60, 60),
            gray4: Color::Rgb(85, 85, 85),
            gray5: Color::Rgb(110, 110, 110),
            muted_text: Color::Rgb(140, 140, 140),
        }
    }

    /// Fallback palette for light terminals.
    #[must_use]
    pub fn default_light() -> Self {
        Self {
            is_dark: false,
            background: Color::Reset,
            gray1: Color::Rgb(240, 240, 240),
            gray2: Color::Rgb(220, 220, 220),
            gray3: Color::Rgb(195, 195, 195),
            gray4: Color::Rgb(165, 165, 165),
            gray5: Color::Rgb(135, 135, 135),
            muted_text: Color::Rgb(100, 100, 100),
        }
    }

    fn dark_palette(r: u8, g: u8, b: u8, lum: f32) -> Self {
        let gray = |level: f32| -> Color {
            if lum < 0.04 {
                let v = (level * 255.0) as u8;
                Color::Rgb(v, v, v)
            } else {
                let ratio = 1.0 + level / lum.max(0.01);
                Color::Rgb(
                    (r as f32 * ratio).min(255.0) as u8,
                    (g as f32 * ratio).min(255.0) as u8,
                    (b as f32 * ratio).min(255.0) as u8,
                )
            }
        };

        let muted_val = if lum < 0.04 {
            140
        } else {
            (130.0 + lum * 80.0).min(180.0) as u8
        };

        Self {
            is_dark: true,
            background: Color::Rgb(r, g, b),
            gray1: gray(0.04),
            gray2: gray(0.08),
            gray3: gray(0.12),
            gray4: gray(0.18),
            gray5: gray(0.25),
            muted_text: Color::Rgb(muted_val, muted_val, muted_val),
        }
    }

    fn light_palette(r: u8, g: u8, b: u8, lum: f32) -> Self {
        let gray = |level: f32| -> Color {
            if lum > 0.96 {
                let v = (255.0 * (1.0 - level)) as u8;
                Color::Rgb(v, v, v)
            } else {
                let ratio = 1.0 - level / (1.0 - lum).clamp(0.01, 1.0) * 0.5;
                Color::Rgb(
                    (r as f32 * ratio).max(0.0) as u8,
                    (g as f32 * ratio).max(0.0) as u8,
                    (b as f32 * ratio).max(0.0) as u8,
                )
            }
        };

        let muted_val = if lum > 0.96 {
            100
        } else {
            (120.0 - lum * 40.0).max(80.0) as u8
        };

        Self {
            is_dark: false,
            background: Color::Rgb(r, g, b),
            gray1: gray(0.03),
            gray2: gray(0.06),
            gray3: gray(0.12),
            gray4: gray(0.20),
            gray5: gray(0.30),
            muted_text: Color::Rgb(muted_val, muted_val, muted_val),
        }
    }
}

#[must_use]
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
}

fn query_background() -> Option<(u8, u8, u8)> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return None;
    }

    crossterm::terminal::enable_raw_mode().ok()?;
    let result = query_osc11();
    drain_stdin();
    let _ = crossterm::terminal::disable_raw_mode();
    result
}

fn query_osc11() -> Option<(u8, u8, u8)> {
    let mut stdout = io::stdout();
    stdout.write_all(b"\x1b]11;?\x07").ok()?;
    stdout.flush().ok()?;

    let start = Instant::now();
    let timeout = Duration::from_millis(100);
    let mut collected = Vec::new();

    while start.elapsed() < timeout {
        let mut buf = [0u8; 128];
        if let Ok(n) = read_stdin_with_timeout(&mut buf, Duration::from_millis(10))
            && n > 0
        {
            collected.extend_from_slice(&buf[..n]);
            if let Some(rgb) = parse_osc11(&collected) {
                return Some(rgb);
            }
        }
    }

    parse_osc11(&collected)
}

#[cfg(unix)]
fn read_stdin_with_timeout(buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
    use std::os::unix::io::AsRawFd;

    let fd = io::stdin().as_raw_fd();
    let timeout_ms = timeout.as_millis() as i32;

    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };

    // SAFETY: poll with a single fd is safe, we pass valid pointer and count
    let poll_result = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };

    if poll_result <= 0 {
        return Ok(0);
    }

    if pollfd.revents & libc::POLLIN == 0 {
        return Ok(0);
    }

    // Data is available, read it
    io::stdin().read(buf)
}

#[cfg(not(unix))]
fn read_stdin_with_timeout(_buf: &mut [u8], _timeout: Duration) -> io::Result<usize> {
    Ok(0)
}

#[cfg(unix)]
fn read_stdin_nonblocking(buf: &mut [u8]) -> io::Result<usize> {
    use std::os::unix::io::AsRawFd;

    let fd = io::stdin().as_raw_fd();

    // SAFETY: fcntl with F_GETFL only reads flags, no memory safety concerns
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: Setting O_NONBLOCK is a safe operation on a valid fd
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    let result = io::stdin().read(buf);
    // SAFETY: Restoring original flags to leave stdin in its original state
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };

    match result {
        Ok(n) => Ok(n),
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

#[cfg(not(unix))]
fn read_stdin_nonblocking(_buf: &mut [u8]) -> io::Result<usize> {
    Ok(0)
}

fn drain_stdin() {
    let mut buf = [0u8; 256];
    let start = Instant::now();
    let timeout = Duration::from_millis(50);

    while start.elapsed() < timeout {
        while crossterm::event::poll(Duration::from_millis(1)).unwrap_or(false) {
            let _ = crossterm::event::read();
        }

        match read_stdin_nonblocking(&mut buf) {
            Ok(0) => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

fn parse_osc11(data: &[u8]) -> Option<(u8, u8, u8)> {
    let s = String::from_utf8_lossy(data);
    let idx = s.find("rgb:")?;
    let rest = &s[idx + 4..];
    let parts: Vec<&str> = rest.split('/').take(3).collect();
    if parts.len() < 3 {
        return None;
    }

    // OSC 11 returns 16-bit values; extract high byte for 8-bit color
    let r = u16::from_str_radix(parts[0], 16).ok()? >> 8;
    let g = u16::from_str_radix(parts[1], 16).ok()? >> 8;
    let b = u16::from_str_radix(
        parts[2].trim_end_matches(|c: char| !c.is_ascii_hexdigit()),
        16,
    )
    .ok()?
        >> 8;

    Some((r as u8, g as u8, b as u8))
}
