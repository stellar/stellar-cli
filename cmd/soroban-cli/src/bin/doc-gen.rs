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

    std::fs::write(
        out_dir.join("FULL_HELP_DOCS.md"),
        clap_markdown::help_markdown::<soroban_cli::Root>(),
    )?;

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
