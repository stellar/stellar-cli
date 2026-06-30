//! Unified output abstraction for commands that support both human-readable and
//! JSON output.
//!
//! Commands construct an [`Output`] from their `--output` format and the global
//! `--quiet` flag, then route all output through it:
//!
//! * [`Output::readable`] runs its closure only in human-readable mode, handing
//!   it a [`Print`] for progress/status messages and text rendering.
//! * [`Output::json`] / [`Output::json_value`] run only in JSON mode and write
//!   the final machine-readable result to stdout.
//!
//! Output is never buffered: each call writes immediately, so long-running
//! operations stream progress to the terminal instead of holding it silently.

use serde::Serialize;

use crate::print::Print;

/// Build the canonical JSON error envelope for an error: `{ "error": { … } }`.
///
/// The inner object is always present and always has at least a `message`
/// string, so every command renders errors with the same shape. When the error
/// chain contains a JSON-RPC [`ErrorObject`](jsonrpsee_types::ErrorObjectOwned)
/// (whose own `Display` is just its debug representation), its structured
/// `{ code, message, data? }` form is used; every other error falls back to its
/// `Display` as the `message`.
#[must_use]
pub fn error_json(err: &(dyn std::error::Error + 'static)) -> serde_json::Value {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(source) = current {
        if let Some(object) = source.downcast_ref::<jsonrpsee_types::ErrorObjectOwned>() {
            if let Ok(value) = serde_json::to_value(object) {
                return serde_json::json!({ "error": value });
            }
        }
        current = source.source();
    }

    serde_json::json!({ "error": { "message": err.to_string() } })
}

/// The format a command should render its output in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// Human-readable output.
    Readable,
    /// Compact, single-line JSON.
    Json,
    /// Pretty-printed, multi-line JSON.
    JsonFormatted,
}

impl Format {
    /// Whether this format is one of the JSON variants.
    #[must_use]
    pub fn is_json(self) -> bool {
        matches!(self, Format::Json | Format::JsonFormatted)
    }
}

/// Wraps the output [`Format`] and a [`Print`] (carrying `--quiet`) so commands
/// can emit human-readable and JSON output through a single value.
#[derive(Clone)]
pub struct Output {
    format: Format,
    print: Print,
}

impl Output {
    /// Create an `Output` for the given format, honoring the global `quiet` flag
    /// for human-readable output.
    #[must_use]
    pub fn new(format: Format, quiet: bool) -> Self {
        Self {
            format,
            print: Print::new(quiet),
        }
    }

    /// Whether the output format is JSON-based.
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.format.is_json()
    }

    /// Whether JSON output should be compact (single-line) rather than pretty.
    #[must_use]
    pub fn compact_json(&self) -> bool {
        self.format == Format::Json
    }

    /// The underlying [`Print`]. Output written through this is gated only by
    /// `--quiet`, not by the output format, so it is still emitted in JSON mode
    /// (to stderr). Use it for diagnostics that remain relevant alongside JSON;
    /// prefer [`Output::readable`] for human-readable output that should be
    /// suppressed entirely in JSON mode.
    #[must_use]
    pub fn print(&self) -> &Print {
        &self.print
    }

    /// Run `f` with a [`Print`] only when rendering human-readable output. A
    /// no-op in JSON mode.
    pub fn readable<F: FnOnce(&Print)>(&self, f: F) {
        if !self.format.is_json() {
            f(&self.print);
        }
    }

    /// Run `f` only when rendering JSON output. A no-op in human-readable mode.
    /// Use this for streaming or custom JSON (e.g. NDJSON); for a single value
    /// prefer [`Output::json_value`].
    pub fn json<F: FnOnce(&Output)>(&self, f: F) {
        if self.format.is_json() {
            f(self);
        }
    }

    /// Serialize `value` to stdout as the final JSON result, compact or
    /// pretty-printed depending on the format. A no-op in human-readable mode.
    ///
    /// # Errors
    /// If `value` fails to serialize.
    pub fn json_value<T: Serialize>(&self, value: &T) -> Result<(), serde_json::Error> {
        match self.format {
            Format::Json => println!("{}", serde_json::to_string(value)?),
            Format::JsonFormatted => println!("{}", serde_json::to_string_pretty(value)?),
            Format::Readable => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn format_is_json() {
        assert!(!Format::Readable.is_json());
        assert!(Format::Json.is_json());
        assert!(Format::JsonFormatted.is_json());
    }

    #[test]
    fn readable_runs_only_in_readable_mode() {
        for (format, expected) in [
            (Format::Readable, true),
            (Format::Json, false),
            (Format::JsonFormatted, false),
        ] {
            let output = Output::new(format, false);
            let ran = Cell::new(false);
            output.readable(|_| ran.set(true));
            assert_eq!(ran.get(), expected, "format: {format:?}");
        }
    }

    #[test]
    fn json_runs_only_in_json_mode() {
        for (format, expected) in [
            (Format::Readable, false),
            (Format::Json, true),
            (Format::JsonFormatted, true),
        ] {
            let output = Output::new(format, false);
            let ran = Cell::new(false);
            output.json(|_| ran.set(true));
            assert_eq!(ran.get(), expected, "format: {format:?}");
        }
    }

    #[test]
    fn compact_json_only_for_compact_format() {
        assert!(Output::new(Format::Json, false).compact_json());
        assert!(!Output::new(Format::JsonFormatted, false).compact_json());
        assert!(!Output::new(Format::Readable, false).compact_json());
    }

    #[test]
    fn error_json_uses_structured_rpc_error_object() {
        let obj = jsonrpsee_types::ErrorObject::owned(-32603, "DB is empty", None::<()>);
        assert_eq!(
            error_json(&obj),
            serde_json::json!({ "error": { "code": -32603, "message": "DB is empty" } }),
        );
    }

    #[test]
    fn error_json_walks_the_source_chain() {
        #[derive(Debug)]
        struct Wrapper(jsonrpsee_types::ErrorObjectOwned);
        impl std::fmt::Display for Wrapper {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "wrapper")
            }
        }
        impl std::error::Error for Wrapper {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let obj = jsonrpsee_types::ErrorObject::owned(-32000, "boom", None::<()>);
        assert_eq!(
            error_json(&Wrapper(obj)),
            serde_json::json!({ "error": { "code": -32000, "message": "boom" } }),
        );
    }

    #[test]
    fn error_json_falls_back_to_message_for_other_errors() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        assert_eq!(
            error_json(&err),
            serde_json::json!({ "error": { "message": "nope" } }),
        );
    }
}
