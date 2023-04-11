use std::process::Command;
use which::which;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Plugin not provided. Should be `soroban plugin` for a binary `soroban-plugin`")]
    MissingSubcommand,
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(
        r#"no such command: `{0}`
        
        {1}View all installed plugins with `soroban --list`"#
    )]
    ExecutableNotFound(String, String),
    #[error(transparent)]
    Which(#[from] which::Error),
}

pub fn run() -> Result<(), Error> {
    let (name, args) = {
        let mut args = std::env::args().skip(1);
        let name = args.next().ok_or(Error::MissingSubcommand)?;
        (name, args)
    };
    let bin = which(format!("soroban-{name}")).map_err(|_| {
        let suggestion = if let Ok(bins) = list() {
            let suggested_name = bins
                .iter()
                .map(|b| (b, strsim::jaro_winkler(&name, b)))
                .filter(|(_, i)| *i > 0.5f64)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(a, _)| a.to_string())
                .unwrap_or_default();
            if suggested_name.is_empty() {
                suggested_name
            } else {
                format!(
                    r#"Did you mean `{suggested_name}`?
        "#
                )
            }
        } else {
            String::new()
        };
        Error::ExecutableNotFound(name, suggestion)
    })?;
    std::process::exit(
        Command::new(bin)
            .args(args)
            .spawn()?
            .wait()?
            .code()
            .unwrap(),
    );
}

pub fn list() -> Result<Vec<String>, Error> {
    let re = regex::Regex::new(r"^soroban-").unwrap();
    Ok(which::which_re(re)?
        .filter_map(|b| b.file_name()?.to_str().map(ToString::to_string))
        .map(|s| s.replace("soroban-", ""))
        .collect())
}
