fn main() {
    crate_git_revision::init();
    let w = &mut std::io::stdout();
    __set_example_contracts(w).unwrap();
}

fn __set_example_contracts(w: &mut impl std::io::Write) -> std::io::Result<()> {
    let example_contracts = get_example_contracts().unwrap().join(",");
    writeln!(w, "cargo:rustc-env=EXAMPLE_CONTRACTS={example_contracts}")?;
    Ok(())
}

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/stellar/soroban-examples/git/trees/main?recursive=1";

#[derive(serde::Deserialize, Debug)]
struct RepoPath {
    path: String,
    #[serde(rename = "type")]
    type_field: String,
}

#[derive(serde::Deserialize, Debug)]
struct ReqBody {
    tree: Vec<RepoPath>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to complete get request")]
    UreqError(#[from] Box<ureq::Error>),

    #[error("Io error: {0}")]
    IoError(#[from] std::io::Error),
}

fn get_example_contracts() -> Result<Vec<String>, Error> {
    let body: ReqBody = ureq::get(GITHUB_API_URL)
        .call()
        .map_err(Box::new)?
        .into_json()?;
    let mut valid_examples = Vec::new();
    for item in body.tree {
        if item.type_field == "blob"
            || item.path.starts_with('.')
            || item.path.contains('/')
            || item.path == "hello_world"
        {
            continue;
        }

        valid_examples.push(item.path);
    }

    Ok(valid_examples)
}
