//! garcas - Standalone CAS command-line interface
//!
//! A REPL and expression evaluator using the garcalc-cas engine.
//!
//! Usage:
//!   garcas                  # Start interactive REPL
//!   garcas -e "2+2"         # Evaluate expression
//!   garcas --exact          # Use exact/symbolic mode

use std::io::{self, BufRead, Write};

use anyhow::{Context, Result};
use clap::Parser;
use garcalc_cas::{eval::AngleMode, Evaluator};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "garcas")]
#[command(about = "Computer Algebra System CLI")]
#[command(version)]
struct Args {
    /// Expression to evaluate (if not provided, starts REPL)
    #[arg(short, long)]
    expr: Option<String>,

    /// Use exact/symbolic mode (keep undefined symbols)
    #[arg(long)]
    exact: bool,

    /// Angle mode: radians or degrees
    #[arg(long, default_value = "radians")]
    angle: String,

    /// Suppress prompts (for scripting)
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let args = Args::parse();

    let angle_mode = match args.angle.to_lowercase().as_str() {
        "deg" | "degrees" => AngleMode::Degrees,
        _ => AngleMode::Radians,
    };

    let evaluator = Evaluator::new()
        .with_exact_mode(args.exact)
        .with_angle_mode(angle_mode);

    if let Some(expr) = args.expr {
        // Single expression mode
        let result = evaluate(&evaluator, &expr)?;
        println!("{result}");
    } else {
        // REPL mode
        repl(evaluator, args.quiet)?;
    }

    Ok(())
}

fn evaluate(evaluator: &Evaluator, input: &str) -> Result<String> {
    let expr = garcalc_cas::parser::parse(input).context("parse error")?;
    let result = evaluator.eval(&expr).context("evaluation error")?;
    Ok(result.to_string())
}

fn repl(mut evaluator: Evaluator, quiet: bool) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    if !quiet {
        println!("garcas - Computer Algebra System");
        println!("Type 'help' for commands, 'quit' to exit\n");
    }

    for line in stdin.lock().lines() {
        let line = line?;
        let input = line.trim();

        if input.is_empty() {
            continue;
        }

        // Handle commands
        match input.to_lowercase().as_str() {
            "quit" | "exit" | "q" => break,
            "help" | "?" => {
                print_help();
                continue;
            }
            "clear" => {
                evaluator.clear_vars();
                if !quiet {
                    println!("Variables cleared");
                }
                continue;
            }
            "vars" => {
                println!("(variable listing not yet implemented)");
                continue;
            }
            _ => {}
        }

        // Check for variable assignment: x = expr
        if let Some((name, value_str)) = input.split_once(":=") {
            let name = name.trim();
            let value_str = value_str.trim();
            match evaluate(&evaluator, value_str) {
                Ok(value) => {
                    if let Ok(expr) = garcalc_cas::parser::parse(&value) {
                        evaluator.set_var(name, expr);
                        if !quiet {
                            println!("{name} := {value}");
                        }
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
            continue;
        }

        // Evaluate expression
        match evaluate(&evaluator, input) {
            Ok(result) => println!("{result}"),
            Err(e) => eprintln!("Error: {e}"),
        }

        if !quiet {
            print!("> ");
            stdout.flush()?;
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"
Commands:
  quit, exit, q    Exit the REPL
  help, ?          Show this help
  clear            Clear all variables
  vars             List defined variables

Variable assignment:
  x := 5           Assign value to variable
  f := x^2 + 1     Define expression

Operators:
  +, -, *, /       Basic arithmetic
  ^                Power/exponentiation
  !                Factorial

Functions:
  Trigonometric:   sin, cos, tan, asin, acos, atan
  Hyperbolic:      sinh, cosh, tanh, asinh, acosh, atanh
  Exponential:     exp, ln, log, log10, log2
  Roots:           sqrt, cbrt
  Other:           abs, floor, ceil, round, sign
  Number theory:   factorial, gcd, lcm
  Aggregation:     min, max

Constants:
  pi, e, i, infinity

Calculus (symbolic):
  diff(expr, x)              Derivative
  diff(expr, x, 2)           Second derivative
  integrate(expr, x)         Indefinite integral
  integrate(expr, x, a, b)   Definite integral
  lim(expr, x, a)            Limit
  sum(expr, n, a, b)         Summation
  product(expr, n, a, b)     Product

Examples:
  2 + 3 * 4
  sin(pi/2)
  diff(x^2, x)
  5!
  sqrt(-1)
"#
    );
}
