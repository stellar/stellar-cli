use std::{
    env,
    path::{Path, PathBuf},
};

type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError> {
    doc_gen()?;
    Ok(())
}

fn doc_gen() -> std::io::Result<()> {
    let out_dir = project_root();
    let options = clap_markdown::MarkdownOptions::new()
        .show_footer(false)
        .show_table_of_contents(false)
        .title("Stellar CLI Manual".to_string());

    let content = clap_markdown::help_markdown_custom::<soroban_cli::Root>(&options);

    std::fs::write(out_dir.join("FULL_HELP_DOCS.md"), content)?;

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
