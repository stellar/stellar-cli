#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid name: {0}\n only alphanumeric characters, underscores (_), and hyphens (-) are allowed.")]
    InvalidNameCharacters(String),
    #[error("Invalid name: {0}\n names cannot exceed 250 characters or be empty")]
    InvalidNameLength(String),
}

pub fn allowed_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub fn validate_name(s: &str) -> Result<(), Error> {
    if s.is_empty() || s.len() > 250 {
        return Err(Error::InvalidNameLength(s.to_string()));
    }
    if !s.chars().all(allowed_char) {
        return Err(Error::InvalidNameCharacters(s.to_string()));
    }
    Ok(())
}
