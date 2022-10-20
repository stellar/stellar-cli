use cargo_metadata::Target;
use filetime::FileTime;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

impl Cmd {
    pub fn bin_name(target: &Target) -> String {
        format!("{}.wasm", target.name.replace('-', "_"))
    }

    fn release_or_debug(&self) -> &str {
        if self.cargo.release {
            "release"
        } else {
            "debug"
        }
    }

    pub fn output_bin(&self, target: &Target) -> Result<PathBuf, Error> {
        self.target_dir().map(|t| {
            t.join(format!(
                "{}_opt.wasm",
                Cmd::bin_name(target).trim_end_matches(".wasm")
            ))
        })
    }

    pub fn target_dir(&self) -> Result<PathBuf, Error> {
        Ok(self
            .cargo
            .target_dir()
            .map_err(|_| Error::Cargo("Filed to find target_dir".to_string()))?
            .join("wasm32-unknown-unknown")
            .join(self.release_or_debug()))
    }

    pub fn orig_bin(&self, target: &Target) -> Result<PathBuf, Error> {
        self.target_dir().map(|t| t.join(Cmd::bin_name(target)))
    }

    pub fn should_rebuild(&self, t: &Target) -> Result<bool, Error> {
        let orig_bin = &self.orig_bin(t)?;
        let output_bin = &self.output_bin(t)?;

        Ok(get_time(output_bin)? < get_time(orig_bin)?)
    }
}

fn get_time(path: &Path) -> Result<FileTime, Error> {
    fs::metadata(path)
        .as_ref()
        .map_err(|_| Error::Cargo(format!("failed to time for {}", path.to_string_lossy())))
        .map(FileTime::from_last_modification_time)
}

fn read_module(filename: &PathBuf) -> binaryen::Module {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = Vec::new();
    f.read_to_end(&mut contents)
        .expect("something went wrong reading the file");

    binaryen::Module::read(&contents).expect("something went wrong parsing the file")
}

fn write_module(filename: &PathBuf, wasm: &[u8]) {
    let mut f = File::create(filename).expect("failed to create output");
    f.write_all(wasm).expect("failed to write file");
}

fn optimize(input: &PathBuf, output: &PathBuf) {
    let mut module = read_module(input);
    let codegen_config = binaryen::CodegenConfig {
        optimization_level: 2,
        shrink_level: 2,
        debug_info: true,
    };
    module.optimize(&codegen_config);

    let optimized_wasm = module.write();
    write_module(output, &optimized_wasm);
}
