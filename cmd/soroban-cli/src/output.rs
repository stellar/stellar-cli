use std::fmt::Display;

pub struct Output {
    pub quiet: bool,
}

impl Output {
    pub fn new(quiet: bool) -> Output {
        Output { quiet }
    }

    fn print<T: Display>(&self, icon: &str, message: T) {
        if !self.quiet {
            eprintln!("{icon} {message}");
        }
    }

    pub fn check<T: Display>(&self, message: T) {
        self.print("✅", message);
    }

    pub fn info<T: Display>(&self, message: T) {
        self.print("ℹ️", message);
    }

    pub fn globe<T: Display>(&self, message: T) {
        self.print("🌎", message);
    }

    pub fn link<T: Display>(&self, message: T) {
        self.print("🔗", message);
    }
}
