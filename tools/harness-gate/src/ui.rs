use clap::ValueEnum;
use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

static COLOR_MODE: OnceLock<ColorMode> = OnceLock::new();

pub fn configure(mode: ColorMode) {
    let _ = COLOR_MODE.set(mode);
}

fn color_enabled(stream: Stream) -> bool {
    match COLOR_MODE.get().copied().unwrap_or_default() {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            std::env::var_os("NO_COLOR").is_none()
                && match stream {
                    Stream::Stdout => io::stdout().is_terminal(),
                    Stream::Stderr => io::stderr().is_terminal(),
                }
        }
    }
}

fn paint(text: impl AsRef<str>, code: u8, stream: Stream) -> String {
    let text = text.as_ref();
    if color_enabled(stream) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn heading(text: impl AsRef<str>) -> String {
    paint(text, 36, Stream::Stdout)
}

pub fn pass(text: impl AsRef<str>) -> String {
    paint(text, 32, Stream::Stdout)
}

pub fn warning(text: impl AsRef<str>) -> String {
    paint(text, 33, Stream::Stdout)
}

pub fn failure(text: impl AsRef<str>) -> String {
    paint(text, 31, Stream::Stdout)
}

pub fn error(text: impl AsRef<str>) -> String {
    paint(text, 31, Stream::Stderr)
}

pub struct Progress {
    total: usize,
    complete: usize,
    enabled: bool,
    finished: bool,
}

impl Progress {
    pub fn new(total: usize) -> Self {
        Self {
            total: total.max(1),
            complete: 0,
            enabled: io::stderr().is_terminal(),
            finished: false,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&self) {
        if self.enabled {
            eprint!("\r\x1b[2K");
            io::stderr().flush().ok();
        }
    }

    pub fn complete(&mut self) {
        self.complete += 1;
    }

    pub fn finish(&mut self) {
        if self.enabled {
            self.render("complete");
            eprintln!();
        }
        self.finished = true;
    }

    fn render(&self, label: &str) {
        let width = 20;
        let filled = width * self.complete / self.total;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(width - filled));
        let prefix = paint("Progress", 36, Stream::Stderr);
        eprint!(
            "\r\x1b[2K{prefix} [{bar}] {}/{} {label}",
            self.complete, self.total
        );
        io::stderr().flush().ok();
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if self.enabled && !self.finished {
            self.clear();
            eprintln!();
        }
    }
}
