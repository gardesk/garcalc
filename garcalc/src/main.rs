//! garcalc - TI-Nspire-like calculator for gardesk
//!
//! A full-featured graphical calculator with CAS, graphing,
//! geometry, spreadsheet, and notes capabilities.

mod app;
mod config;
mod ipc;
mod ui;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "garcalc")]
#[command(about = "TI-Nspire-like calculator for gardesk")]
#[command(version)]
struct Args {
    /// Run as popup (centered, closes on focus loss)
    #[arg(short, long)]
    popup: bool,

    /// Run as daemon (listen for IPC commands)
    #[arg(short, long)]
    daemon: bool,

    /// Initial mode (calculator, graph, graph3d, geometry, spreadsheet, notes)
    #[arg(short, long, default_value = "calculator")]
    mode: String,

    /// Expression to evaluate (exits after evaluation)
    #[arg(short, long)]
    eval: Option<String>,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Quick eval mode - just evaluate and exit
    if let Some(expr) = args.eval {
        return eval_and_exit(&expr);
    }

    if args.daemon {
        run_daemon()
    } else {
        run_calculator(&args)
    }
}

fn eval_and_exit(expr: &str) -> Result<()> {
    let parsed = garcalc_cas::parser::parse(expr)?;
    let evaluator = garcalc_cas::Evaluator::new();
    let result = evaluator.eval(&parsed)?;
    println!("{result}");
    Ok(())
}

fn run_daemon() -> Result<()> {
    tracing::info!("Starting garcalc daemon");

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let mut server = ipc::IpcServer::new().await?;
        server.run().await
    })
}

fn run_calculator(args: &Args) -> Result<()> {
    let mode = match args.mode.to_lowercase().as_str() {
        "calc" | "calculator" => garcalc_ipc::Mode::Calculator,
        "graph" | "graphing" | "2d" => garcalc_ipc::Mode::Graph,
        "graph3d" | "3d" | "surface" => garcalc_ipc::Mode::Graph3D,
        "geo" | "geometry" => garcalc_ipc::Mode::Geometry,
        "sheet" | "spreadsheet" => garcalc_ipc::Mode::Spreadsheet,
        "notes" | "note" => garcalc_ipc::Mode::Notes,
        _ => {
            tracing::warn!("Unknown mode '{}', defaulting to calculator", args.mode);
            garcalc_ipc::Mode::Calculator
        }
    };

    let mut app = app::App::new(mode, args.popup)?;
    app.run()
}
