use std::io::Write as _;
use termcolor::{Buffer, ColorSpec, WriteColor as _};

fn colored(s: &str, spec: &ColorSpec) -> String {
    let mut buf = Buffer::ansi();
    let _ = buf.set_color(spec);
    let _ = write!(buf, "{s}");
    let _ = buf.reset();
    String::from_utf8(buf.into_inner()).unwrap_or_else(|_| s.to_string())
}

pub fn gray(s: &str) -> String {
    let mut spec = ColorSpec::new();
    spec.set_dimmed(true);
    colored(s, &spec)
}

pub fn green(s: &str) -> String {
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(termcolor::Color::Green)).set_bold(true);
    colored(s, &spec)
}

pub fn red(s: &str) -> String {
    let mut spec = ColorSpec::new();
    spec.set_fg(Some(termcolor::Color::Red)).set_bold(true);
    colored(s, &spec)
}
