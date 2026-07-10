//! `axiom-cli`: the command-line interface for Axiom IR.
//!
//! Every command returns an [`Outcome`] carrying both a human-readable report and a
//! structured JSON value, so the tool is equally usable by humans and by pipelines.
//! Exit codes are stable: `0` success, `1` command failure, `2` usage error.

pub mod commands;
pub mod demos;

use serde_json::Value;

/// The result of running a CLI command.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Stable process exit code.
    pub exit: i32,
    /// Human-readable report (used unless `--json` is set).
    pub human: String,
    /// Structured representation (used when `--json` is set).
    pub json: Value,
}

impl Outcome {
    pub fn ok(human: String, json: Value) -> Outcome {
        Outcome {
            exit: 0,
            human,
            json,
        }
    }
    pub fn fail(human: String, json: Value) -> Outcome {
        Outcome {
            exit: 1,
            human,
            json,
        }
    }
    pub fn usage(human: String) -> Outcome {
        Outcome {
            exit: 2,
            human,
            json: Value::Null,
        }
    }
}
