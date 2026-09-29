use std::{
    fs,
    io::{self, BufRead, BufReader, IsTerminal},
    path::Path,
    process::{Command, Stdio},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use url::Url;

use super::Error;
use crate::{
    commands::{global, plugin, HEADING_ARCHIVE},
    config::{data, network},
    print::Print,
    utils::{http, url::redact_url, XDR_DEPTH_LIMIT},
    xdr::{Frame, LedgerCloseMeta, Limited, Limits, ReadXdr, WriteXdr},
};

const CONFIG_FILE: &str = "stellar-core.cfg";
const LOG_FILE: &str = "stellar-core.log";
const META_FILE: &str = "meta.xdr";
const LAST_LEDGER_FILE: &str = "last-ledger";
const LOCK_FILE: &str = "lock";

/// The most ledgers to replay to continue from the last ledger replayed.
/// Replaying more takes longer than starting over from the ledger state at the
/// checkpoint before the ledger.
const MAX_LEDGERS_TO_CONTINUE: u32 = 640;

#[derive(Debug, Clone, clap::Args)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    /// Archive URL
    #[arg(
        long,
        help_heading = HEADING_ARCHIVE,
        env = "STELLAR_ARCHIVE_URL",
        hide_env_values = true
    )]
    pub archive_url: Option<Url>,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    /// JSON output (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output
    JsonFormatted,
    /// Base64 encoded XDR output
    Xdr,
}

impl OutputFormat {
    pub fn print(self, value: &(impl Serialize + WriteXdr)) -> Result<(), Error> {
        let output = match self {
            OutputFormat::Json => serde_json::to_string(value)?,
            OutputFormat::JsonFormatted => serde_json::to_string_pretty(value)?,
            OutputFormat::Xdr => value.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
        };
        println!("{output}");
        Ok(())
    }
}

impl Args {
    /// Replays the ledger with stellar-core and returns its meta.
    ///
    /// stellar-core's state is kept in the cache directory between runs, so
    /// that replaying a ledger shortly after the last one replayed continues
    /// from it, and the meta of each ledger replayed is kept so that it is
    /// only replayed once.
    pub async fn replay(
        &self,
        ledger: u32,
        global_args: &global::Args,
    ) -> Result<LedgerCloseMeta, Error> {
        let print = Print::new(global_args.quiet);
        let network = self.network.resolve(&global_args.locator, false)?;
        let passphrase = &network.network_passphrase;
        let dir = data::cache_dir()?
            .join("replay")
            .join(hex::encode(Sha256::digest(passphrase)));
        let meta_dir = dir.join("meta");
        fs::create_dir_all(&meta_dir)?;

        // Replays of the network share the directory, so replay one at a time.
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(LOCK_FILE))?;
        if lock.try_lock().is_err() {
            print.infoln("Waiting for another replay to finish…");
            lock.lock()?;
        }

        let cached = meta_dir.join(format!("{ledger}.xdr"));
        if cached.exists() {
            print.infoln(format!("Using ledger {ledger} replayed previously"));
            let meta =
                LedgerCloseMeta::from_xdr(fs::read(&cached)?, Limits::depth(XDR_DEPTH_LIMIT))?;
            return Ok(meta);
        }

        let bin = plugin::default::find_bin("core").map_err(|_| Error::StellarCoreNotFound)?;
        let archive_url = self
            .archive_url
            .clone()
            .or_else(|| network::default_archive_url(passphrase))
            .ok_or(Error::ArchiveUrlNotConfigured)?;
        let latest = latest_ledger_in_archive(&archive_url).await?;
        if ledger > latest {
            return Err(Error::LedgerNotInArchive { ledger, latest });
        }

        fs::write(dir.join(CONFIG_FILE), config(passphrase, &archive_url))?;
        // stellar-core appends to the log and meta files.
        let _ = fs::remove_file(dir.join(LOG_FILE));
        let _ = fs::remove_file(dir.join(META_FILE));

        // Forget the last ledger replayed while stellar-core runs, so that the
        // next replay starts over if it stops part way.
        let last_ledger_file = dir.join(LAST_LEDGER_FILE);
        let last_ledger = fs::read_to_string(&last_ledger_file)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        let _ = fs::remove_file(&last_ledger_file);

        print.infoln(format!(
            "Replaying ledger {ledger} with {} in {}",
            bin.display(),
            dir.display()
        ));
        let verbose = global_args.verbose || global_args.very_verbose;
        let run = |args: &[&str]| run_stellar_core(&print, verbose, &bin, &dir, args);
        match last_ledger {
            Some(last) if last < ledger && ledger - last <= MAX_LEDGERS_TO_CONTINUE => {
                print.infoln(format!("Continuing from ledger {last} replayed previously"));
            }
            _ => {
                print.infoln(
                    "Starting from the checkpoint before the ledger, downloading its ledger state can take several minutes",
                );
                run(&["new-db"])?;
            }
        }
        run(&[
            "catchup",
            &format!("{ledger}/1"),
            "--metadata-output-stream",
            META_FILE,
        ])?;
        fs::write(&last_ledger_file, ledger.to_string())?;

        let meta = read_meta(fs::File::open(dir.join(META_FILE))?, ledger)?;
        let tmp = cached.with_extension("tmp");
        fs::write(&tmp, meta.to_xdr(Limits::depth(XDR_DEPTH_LIMIT))?)?;
        fs::rename(&tmp, &cached)?;
        let _ = fs::remove_file(dir.join(META_FILE));

        print.checkln(format!("Replayed ledger {ledger}"));
        Ok(meta)
    }
}

/// Returns the stellar-core config for replaying ledgers of the network from
/// the history archive.
fn config(network_passphrase: &str, archive_url: &Url) -> String {
    let get = format!(
        "curl -sf {}/{{0}} -o {{1}}",
        archive_url.as_str().trim_end_matches('/')
    );
    format!(
        r#"NETWORK_PASSPHRASE={}
ENABLE_SOROBAN_DIAGNOSTIC_EVENTS=true
HTTP_PORT=0
LOG_FILE_PATH="{LOG_FILE}"

# stellar-core requires a quorum set to start, but catchup doesn't use it
# because it doesn't connect to other nodes, so the validator is a placeholder.
UNSAFE_QUORUM=true
FAILURE_SAFETY=0
[QUORUM_SET]
THRESHOLD_PERCENT=100
VALIDATORS=["GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"]

[HISTORY.archive]
get={}
"#,
        toml::Value::from(network_passphrase),
        toml::Value::from(get),
    )
}

/// Returns the latest ledger in the history archive.
async fn latest_ledger_in_archive(archive_url: &Url) -> Result<u32, Error> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct HistoryArchiveState {
        current_ledger: u32,
    }
    let url = format!(
        "{}/.well-known/stellar-history.json",
        archive_url.as_str().trim_end_matches('/')
    );
    let get = async {
        http::client()
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<HistoryArchiveState>()
            .await
    };
    let has = get.await.map_err(|e| Error::GettingHistory {
        url: redact_url(&url),
        error: e.without_url(),
    })?;
    Ok(has.current_ledger)
}

/// Runs stellar-core in the directory, printing its progress and errors, or
/// all of its log when verbose.
fn run_stellar_core(
    print: &Print,
    verbose: bool,
    bin: &Path,
    dir: &Path,
    args: &[&str],
) -> Result<(), Error> {
    let mut child = Command::new(bin)
        .args(args)
        .args(["--conf", CONFIG_FILE, "--console"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(Error::RunningStellarCore)?;

    // stellar-core logs to stderr. Progress lines replace the previous one
    // when stderr is a terminal.
    let log = BufReader::new(child.stderr.take().expect("stderr is piped"));
    let replace_progress = io::stderr().is_terminal();
    let mut replace_previous = false;
    for line in log.split(b'\n') {
        let line = line.map_err(Error::RunningStellarCore)?;
        let line = String::from_utf8_lossy(&line);
        if verbose {
            print.println(line);
        } else if let Some((_, progress)) = line
            .split_once("Catching up to ledger ")
            .and_then(|(_, rest)| rest.split_once(": "))
        {
            if replace_previous {
                print.clear_previous_line();
            }
            print.infoln(progress);
            replace_previous = replace_progress;
        } else if line.contains(" ERROR] ") || line.contains(" FATAL] ") {
            print.errorln(line);
            replace_previous = false;
        }
    }

    let status = child.wait().map_err(Error::RunningStellarCore)?;
    if !status.success() {
        return Err(Error::StellarCoreFailed {
            status,
            log: dir.join(LOG_FILE),
        });
    }
    Ok(())
}

/// Reads the meta of the ledger from the stream of meta stellar-core wrote.
fn read_meta(stream: impl io::Read, ledger: u32) -> Result<LedgerCloseMeta, Error> {
    let mut stream = Limited::new(stream, Limits::depth(XDR_DEPTH_LIMIT));
    for frame in Frame::<LedgerCloseMeta>::read_xdr_iter(&mut stream) {
        let Frame(meta) = frame?;
        if ledger_seq(&meta) == ledger {
            return Ok(meta);
        }
    }
    Err(Error::LedgerMissingFromMeta(ledger))
}

fn ledger_seq(meta: &LedgerCloseMeta) -> u32 {
    match meta {
        LedgerCloseMeta::V0(m) => m.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V1(m) => m.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V2(m) => m.ledger_header.header.ledger_seq,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::xdr::LedgerCloseMetaV2;

    fn meta(ledger_seq: u32) -> LedgerCloseMeta {
        let mut meta = LedgerCloseMetaV2::default();
        meta.ledger_header.header.ledger_seq = ledger_seq;
        LedgerCloseMeta::V2(meta)
    }

    // Frames each meta the way stellar-core writes them to the meta stream.
    fn stream(metas: &[LedgerCloseMeta]) -> Vec<u8> {
        let mut stream = Vec::new();
        for meta in metas {
            let xdr = meta.to_xdr(Limits::none()).unwrap();
            let len = u32::try_from(xdr.len()).unwrap();
            stream.extend((len | 0x8000_0000).to_be_bytes());
            stream.extend(xdr);
        }
        stream
    }

    #[test]
    fn test_read_meta() {
        let stream = stream(&[meta(10), meta(11), meta(12)]);
        let meta = read_meta(stream.as_slice(), 11).unwrap();
        assert_eq!(ledger_seq(&meta), 11);
    }

    #[test]
    fn test_read_meta_missing_ledger() {
        let stream = stream(&[meta(10), meta(11)]);
        let err = read_meta(stream.as_slice(), 12).unwrap_err();
        assert!(matches!(err, Error::LedgerMissingFromMeta(12)));
    }

    #[test]
    fn test_config() {
        let passphrase = r#"Network "quoted" ; 2026"#;
        let archive_url = Url::parse("https://history.example.org/archive/").unwrap();
        let config: toml::Table = toml::from_str(&config(passphrase, &archive_url)).unwrap();
        assert_eq!(config["NETWORK_PASSPHRASE"].as_str(), Some(passphrase));
        assert_eq!(
            config["HISTORY"]["archive"]["get"].as_str(),
            Some("curl -sf https://history.example.org/archive/{0} -o {1}")
        );
    }
}
