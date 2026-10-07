use std::{
    ffi::OsStr,
    fmt::{self, Display, Formatter},
    marker::PhantomData,
    str::FromStr,
};

use clap::{
    builder::{TypedValueParser, ValueParserFactory},
    error::ErrorKind,
    Arg, Command,
};

/// The command-line argument a value was parsed from (e.g. `--contract-id`),
/// captured by clap so an error can name the argument instead of echoing the
/// value, which may be a secret pasted in the wrong place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArgName(Option<String>);

impl ArgName {
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.0.is_some()
    }
}

impl From<&Arg> for ArgName {
    fn from(arg: &Arg) -> Self {
        let name = if let Some(long) = arg.get_long() {
            // Visible aliases too, since the value may have been given under
            // one, e.g. `--contract-id/--id`.
            std::iter::once(long)
                .chain(arg.get_visible_aliases().unwrap_or_default())
                .map(|name| format!("--{name}"))
                .collect::<Vec<_>>()
                .join("/")
        } else if let Some(short) = arg.get_short() {
            format!("-{short}")
        } else {
            format!("<{}>", arg.get_id().as_str().to_uppercase())
        };
        ArgName(Some(name))
    }
}

impl Display for ArgName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_deref().unwrap_or("value"))
    }
}

/// A value plus the argument it was parsed from. Use as the type of a clap
/// field (e.g. `Named<UnresolvedContract>`) so errors resolving it can name the
/// argument.
#[derive(Clone, Debug, Default)]
pub struct Named<T> {
    pub value: T,
    pub arg: ArgName,
}

impl<T> From<T> for Named<T> {
    fn from(value: T) -> Self {
        Named {
            value,
            arg: ArgName::default(),
        }
    }
}

impl<T: FromStr> FromStr for Named<T> {
    type Err = T::Err;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Named {
            value: value.parse()?,
            arg: ArgName::default(),
        })
    }
}

impl<T> ValueParserFactory for Named<T>
where
    T: FromStr + Clone + Send + Sync + 'static,
    T::Err: Display,
{
    type Parser = NamedParser<T>;

    fn value_parser() -> Self::Parser {
        NamedParser(PhantomData)
    }
}

pub struct NamedParser<T>(PhantomData<T>);

impl<T> Clone for NamedParser<T> {
    fn clone(&self) -> Self {
        NamedParser(PhantomData)
    }
}

impl<T> TypedValueParser for NamedParser<T>
where
    T: FromStr + Clone + Send + Sync + 'static,
    T::Err: Display,
{
    type Value = Named<T>;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Named<T>, clap::Error> {
        let value = value
            .to_str()
            .ok_or_else(|| clap::Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd))?;
        Ok(Named {
            value: value
                .parse()
                .map_err(|e| clap::Error::raw(ErrorKind::ValueValidation, e).with_cmd(cmd))?,
            arg: arg.map(ArgName::from).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::Named;
    use crate::config::{locator, UnresolvedContract, UnresolvedScAddress};

    const SECRET: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BX";
    const SEED: &str = "illness spike retreat truth genius clock brain pass fit cave bargain xyzzy";

    #[derive(Parser)]
    struct Cmd {
        #[arg(long = "contract-id", visible_alias = "id")]
        contract_id: Option<Named<UnresolvedContract>>,
        #[arg(long)]
        account: Option<Named<UnresolvedScAddress>>,
        #[arg(long)]
        contracts: Vec<Named<UnresolvedContract>>,
    }

    fn locator() -> (tempfile::TempDir, locator::Args) {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        (dir, locator)
    }

    fn contract_err(args: &[&str]) -> String {
        let (_dir, locator) = locator();
        let cmd = Cmd::parse_from([&["test"], args].concat());
        cmd.contract_id
            .or_else(|| cmd.contracts.into_iter().next())
            .unwrap()
            .resolve_contract_id(&locator, "Test Network")
            .unwrap_err()
            .to_string()
    }

    fn address_err(value: &str) -> String {
        let (_dir, locator) = locator();
        let cmd = Cmd::parse_from(["test", "--account", value]);
        cmd.account
            .unwrap()
            .resolve(&locator, "Test Network", None)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn contract_errors_name_the_arg_not_the_value() {
        for value in ["nosuchalias", SECRET, SEED] {
            assert_eq!(
                contract_err(&["--id", value]),
                "--contract-id/--id: contract not found"
            );
        }
        assert_eq!(
            contract_err(&["--contracts", "nosuchalias"]),
            "--contracts: contract not found"
        );
    }

    #[test]
    fn contract_error_keeps_wasm_hash_hint() {
        assert_eq!(
            contract_err(&["--id", &"ab".repeat(32)]),
            "--contract-id/--id: contract not found; expected a contract address (C...), got a hash"
        );
    }

    #[test]
    fn address_errors_name_the_arg_not_the_value() {
        for value in ["nosuchalias", SECRET, SEED] {
            assert_eq!(address_err(value), "--account: invalid address or alias");
        }
    }

    #[test]
    fn values_parsed_outside_clap_keep_unnamed_errors() {
        let (_dir, locator) = locator();
        let err = "nosuchalias"
            .parse::<Named<UnresolvedContract>>()
            .unwrap()
            .resolve_contract_id(&locator, "Test Network")
            .unwrap_err();
        assert_eq!(err.to_string(), "contract not found: nosuchalias");
    }
}
