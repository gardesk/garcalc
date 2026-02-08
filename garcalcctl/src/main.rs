//! garcalcctl - Control utility for garcalc
//!
//! Sends commands to the running garcalc daemon via IPC.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use garcalc_ipc::{socket_path, Command, Mode, Response};

#[derive(Parser)]
#[command(name = "garcalcctl")]
#[command(about = "Control utility for garcalc")]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: CtlCommand,
}

#[derive(Subcommand)]
enum CtlCommand {
    /// Show the calculator window
    Show,
    /// Hide the calculator window
    Hide,
    /// Toggle window visibility
    Toggle,
    /// Evaluate an expression
    Eval {
        /// Expression to evaluate
        expr: String,
    },
    /// Get or set calculator mode
    Mode {
        /// Mode to set (calculator, graph, geometry, spreadsheet, notes)
        #[arg(value_name = "MODE")]
        mode: Option<String>,
    },
    /// Get daemon status
    Status,
    /// Quit the daemon
    Quit,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let command = match args.command {
        CtlCommand::Show => Command::Show,
        CtlCommand::Hide => Command::Hide,
        CtlCommand::Toggle => Command::Toggle,
        CtlCommand::Eval { expr } => Command::Evaluate { expr },
        CtlCommand::Mode { mode: None } => Command::GetMode,
        CtlCommand::Mode { mode: Some(m) } => {
            let mode = match m.to_lowercase().as_str() {
                "calc" | "calculator" => Mode::Calculator,
                "graph" | "graphing" => Mode::Graph,
                "geo" | "geometry" => Mode::Geometry,
                "sheet" | "spreadsheet" => Mode::Spreadsheet,
                "notes" | "note" => Mode::Notes,
                _ => {
                    eprintln!("Unknown mode: {m}");
                    eprintln!("Valid modes: calculator, graph, geometry, spreadsheet, notes");
                    std::process::exit(1);
                }
            };
            Command::SetMode { mode }
        }
        CtlCommand::Status => Command::Status,
        CtlCommand::Quit => Command::Quit,
    };

    let response = send_command(&command)?;

    if response.success {
        if let Some(data) = response.data {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    } else if let Some(error) = response.error {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }

    Ok(())
}

fn send_command(command: &Command) -> Result<Response> {
    let socket = socket_path();
    let mut stream = UnixStream::connect(&socket)
        .with_context(|| format!("Failed to connect to garcalc at {}", socket.display()))?;

    let json = serde_json::to_string(command)?;
    writeln!(stream, "{json}")?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: Response = serde_json::from_str(&response_line)
        .context("Failed to parse response from garcalc")?;

    Ok(response)
}
