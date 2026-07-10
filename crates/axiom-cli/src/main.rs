//! `axiom` — command-line interface for Axiom IR.

use axiom_cli::commands;
use axiom_cli::Outcome;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "axiom", version, about = "Axiom IR — proof-carrying intermediate representation for machine reasoning", long_about = None)]
struct Cli {
    /// Emit structured JSON instead of human-readable text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and type-check a module, reporting diagnostics.
    Check { module: String },
    /// Format a module (canonical printer). Use --check to verify stability.
    Fmt {
        module: String,
        #[arg(long)]
        check: bool,
    },
    /// Execute a module and report verification + digest.
    Run {
        module: String,
        #[arg(long = "cap", action = clap::ArgAction::Append)]
        caps: Vec<String>,
        #[arg(long)]
        builtin_tool: bool,
        /// Write a receipt log (JSON) for later offline replay.
        #[arg(long)]
        emit_receipts: Option<String>,
    },
    /// Report which claims verify and which are blocked.
    Verify {
        module: String,
        #[arg(long = "cap", action = clap::ArgAction::Append)]
        caps: Vec<String>,
        #[arg(long)]
        builtin_tool: bool,
    },
    /// Explain why a claim exists (operation, evidence, assumptions, obligations).
    Explain { module: String, claim: String },
    /// Print the provenance chain of a claim.
    Trace { module: String, claim: String },
    /// List contradiction witnesses.
    Contradictions { module: String },
    /// Invalidate a node and run the incremental engine.
    Invalidate { module: String, node: String },
    /// Replay a module from a receipt log (no live capabilities required).
    Replay { receipt_log: String },
    /// Structural diff between two executed modules.
    Diff { module_a: String, module_b: String },
    /// Emit the dependency graph (Graphviz DOT).
    Graph { module: String },
    /// Inspect a node by label.
    Inspect { module: String, node: String },
    /// Run the conformance suite.
    Conform {
        #[arg(long, default_value = "conformance")]
        fixtures: String,
    },
    /// Environment and self-test.
    Doctor,
    /// Run a demonstration (1..7 or all).
    Demo { which: String },
}

fn main() {
    let cli = Cli::parse();
    let json = cli.json;
    let outcome: Outcome = dispatch(cli);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome.json).unwrap_or_else(|_| "null".into())
        );
    } else {
        println!("{}", outcome.human);
    }
    std::process::exit(outcome.exit);
}

fn dispatch(cli: Cli) -> Outcome {
    match cli.command {
        Command::Check { module } => commands::check(&module),
        Command::Fmt { module, check } => commands::fmt(&module, check),
        Command::Run {
            module,
            caps,
            builtin_tool,
            emit_receipts,
        } => commands::run(&module, &caps, builtin_tool, emit_receipts.as_deref()),
        Command::Verify {
            module,
            caps,
            builtin_tool,
        } => commands::verify(&module, &caps, builtin_tool),
        Command::Explain { module, claim } => commands::explain(&module, &claim),
        Command::Trace { module, claim } => commands::trace(&module, &claim),
        Command::Contradictions { module } => commands::contradictions(&module),
        Command::Invalidate { module, node } => commands::invalidate(&module, &node),
        Command::Replay { receipt_log } => commands::replay(&receipt_log),
        Command::Diff { module_a, module_b } => commands::diff(&module_a, &module_b),
        Command::Graph { module } => commands::graph(&module),
        Command::Inspect { module, node } => commands::inspect(&module, &node),
        Command::Conform { fixtures } => {
            let report = axiom_conformance::run(&fixtures);
            let json = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
            let human = format!(
                "conformance: {}/{} fixtures passed ({} failed)\n{}",
                report.passed,
                report.total,
                report.failed,
                report.summary()
            );
            let exit = if report.failed == 0 { 0 } else { 1 };
            Outcome { exit, human, json }
        }
        Command::Doctor => commands::doctor(),
        Command::Demo { which } => axiom_cli::demos::run_demo(&which),
    }
}
