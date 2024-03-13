fn main() {
    crate_git_revision::init();
    build_helper::set_example_contracts();
}

mod build_helper {
    use std::{
        fs::{metadata, File, Metadata},
        io::{self, Write},
        path::{Path, PathBuf},
    };

    const GITHUB_API_URL: &str =
        "https://api.github.com/repos/stellar/soroban-examples/git/trees/main?recursive=1";

    pub fn set_example_contracts() {
        let example_contracts = get_example_contracts().unwrap();
        let w = &mut std::io::stdout();
        set_example_contracts_env_var(w, &example_contracts).unwrap();
    }

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

    fn get_example_contracts() -> Result<String, Error> {
        if file_exists(&cached_example_contracts_file_path()) {
            let example_contracts = std::fs::read_to_string(cached_example_contracts_file_path())?;
            return Ok(example_contracts);
        }

        Ok(fetch_and_cache_example_contracts())
    }

    fn fetch_and_cache_example_contracts() -> String {
        let example_contracts = fetch_example_contracts().unwrap().join(",");
        let cached_example_contracts = target_dir().join("example_contracts.txt");

        if let Err(err) = write_cache(&cached_example_contracts, &example_contracts) {
            eprintln!("Error writing cache: {err}");
        }

        example_contracts
    }

    fn fetch_example_contracts() -> Result<Vec<String>, Error> {
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

    fn set_example_contracts_env_var(
        w: &mut impl std::io::Write,
        example_contracts: &str,
    ) -> std::io::Result<()> {
        writeln!(w, "cargo:rustc-env=EXAMPLE_CONTRACTS={example_contracts}")?;
        Ok(())
    }

    fn cached_example_contracts_file_path() -> PathBuf {
        target_dir().join("example_contracts.txt")
    }

    fn target_dir() -> PathBuf {
        project_root().join("target")
    }

    fn project_root() -> PathBuf {
        Path::new(&env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }

    fn write_cache(cache_file_path: &Path, data: &str) -> io::Result<()> {
        // Create or open the cache file
        let mut file = File::create(cache_file_path)?;

        // Write the data to the cache file
        file.write_all(data.as_bytes())?;

        Ok(())
    }

    fn file_exists(file_path: &Path) -> bool {
        metadata(file_path)
            .as_ref()
            .map(Metadata::is_file)
            .unwrap_or(false)
    }
}
