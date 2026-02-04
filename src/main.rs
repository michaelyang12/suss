mod args;
mod config;

use args::Args;
use clap::Parser;
use colored::*;
use std::io::{self, Read};

#[tokio::main]
async fn main() {
    let mut args = Args::parse();

    if args.config {
        println!("config wizard (TODO)");
        return;
    }

    if args.upgrade {
        println!("upgrade (TODO)");
        return;
    }

    // Read from stdin if piped
    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_input = String::new();
        io::stdin().read_to_string(&mut stdin_input).unwrap();
        args.input = stdin_input;
    }

    if args.input.trim().is_empty() {
        eprintln!("{}", "Usage: ss \"error message\" or pipe: cmd 2>&1 | ss".red());
        std::process::exit(1);
    }

    println!("Input: {}", args.input.dimmed());
    println!("(LLM request TODO)");
}
