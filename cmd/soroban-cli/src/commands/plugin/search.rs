use serde::{Deserialize, Serialize};
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::{commands::global, print::Print, utils::http};

use super::super::config::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("unable to retrieve the list of plugins from GitHub")]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd;

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let url =
            "https://api.github.com/search/repositories?q=topic%3Astellar-cli-plugin+fork%3Afalse+archived%3Afalse&per_page=100&sort=stars&order=desc";

        let resp = http::client().get(url).send().await?;
        let search: SearchResponse = resp.json().await?;

        if search.total_count == 0 {
            print.searchln("No plugins found.".to_string());
            return Ok(());
        }

        let wording = if search.total_count == 1 {
            "plugin"
        } else {
            "plugins"
        };

        print.searchln(format!(
            "Found {total} {wording}:",
            total = search.total_count
        ));

        let mut stdout = StandardStream::stdout(ColorChoice::Auto);

        for item in search.items {
            println!();
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
            writeln!(&mut stdout, "  {}", item.full_name)?;
            stdout.reset()?;

            if let Some(description) = item.description {
                writeln!(&mut stdout, "  {description}")?;
            }

            print.blankln(item.html_url.to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResponse {
    total_count: u32,
    incomplete_results: bool,
    items: Vec<Repository>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Repository {
    id: u64,
    name: String,
    full_name: String,
    html_url: String,
    description: Option<String>,
}
