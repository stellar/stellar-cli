use std::{collections::HashMap, path::PathBuf};
use testcontainers::{core::WaitFor, Image, ImageArgs};

const NAME: &str = "docker.io/zondax/builder-zemu";
const TAG: &str = "speculos-3a3439f6b45eca7f56395673caaf434c202e7005";
const TEST_SEED_PHRASE: &str =
    "\"other base behind follow wet put glad muscle unlock sell income october\"";

#[allow(dead_code)]
static ENV: &Map = &Map(phf::phf_map! {
    "BOLOS_SDK"=> "/project/deps/nanos-secure-sdk",
    "BOLOS_ENV" => "/opt/bolos",
    "DISPLAY" => "host.docker.internal:0",
});
struct Map(phf::Map<&'static str, &'static str>);

#[allow(clippy::implicit_hasher)]
impl From<&Map> for HashMap<String, String> {
    fn from(Map(map): &Map) -> Self {
        map.into_iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct Speculos(HashMap<String, String>, HashMap<String, String>);
const DEFAULT_APP_PATH: &str = "/project/app/bin";
impl Speculos {
    #[allow(dead_code)]
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let apps_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("test_fixtures")
            .join("apps");
        let mut volumes = HashMap::new();
        volumes.insert(
            apps_dir.to_str().unwrap().to_string(),
            DEFAULT_APP_PATH.to_string(),
        );
        Speculos(ENV.into(), volumes)
    }
}

#[derive(Debug, Clone)]
pub struct Args {
    pub ledger_device_model: String,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            ledger_device_model: "nanos".to_string(),
        }
    }
}

impl ImageArgs for Args {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        let device_model = self.ledger_device_model.clone();
        let container_elf_path = match device_model.as_str() {
            "nanos" => format!("{DEFAULT_APP_PATH}/stellarNanoSApp.elf"),
            "nanosp" => format!("{DEFAULT_APP_PATH}/stellarNanoSPApp.elf"),
            "nanox" => format!("{DEFAULT_APP_PATH}/stellarNanoXApp.elf"),
            _ => panic!("Unsupported device model"),
        };
        let command_string = format!("/home/zondax/speculos/speculos.py --log-level speculos:DEBUG --color JADE_GREEN --display headless -s {TEST_SEED_PHRASE} -m {device_model}  {container_elf_path}");
        Box::new(vec![command_string].into_iter())
    }
}

impl Image for Speculos {
    type Args = Args;

    fn name(&self) -> String {
        NAME.to_owned()
    }

    fn tag(&self) -> String {
        TAG.to_owned()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("HTTP proxy started...")]
    }

    fn env_vars(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.0.iter())
    }

    fn volumes(&self) -> Box<dyn Iterator<Item = (&String, &String)> + '_> {
        Box::new(self.1.iter())
    }
}
