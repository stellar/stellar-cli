use std::{
    env, fs,
    path::{Path, PathBuf},
};

type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError> {
    doc_gen()?;
    Ok(())
}

fn doc_gen() -> std::io::Result<()> {
    let out_dir = docs_dir();

    fs::create_dir_all(out_dir.clone())?;

    std::fs::write(
        out_dir.join("soroban-cli-full-docs.md"),
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

fn docs_dir() -> PathBuf {
    project_root().join("docs")
}
