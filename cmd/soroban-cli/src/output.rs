pub struct Output {
    pub quiet: bool,
}

impl Output {
    pub fn new(quiet: bool) -> Output {
        Output { quiet }
    }

    fn print(&self, icon: &str, message: &str) {
        if !self.quiet {
            eprintln!("{icon} {message}");
        }
    }

    pub fn check(&self, message: &str) {
        self.print("✅", message);
    }

    pub fn info(&self, message: &str) {
        self.print("ℹ️", message);
    }

    pub fn globe(&self, message: &str) {
        self.print("🌎", message);
    }

    pub fn link(&self, message: &str) {
        self.print("🔗", message);
    }
}
