mod args;
mod client;
mod config;
mod context;
mod setup;

use std::io::{self, BufRead, Read, Write};
use std::process::{Command, Stdio};

use args::Args;
use client::{RequestClient, RequestMode};
use config::Config;
use context::Context;
use clap::Parser;
use colored::*;

#[tokio::main]
async fn main() {
    let mut args = Args::parse();

    if args.upgrade {
        upgrade();
        return;
    }

    if args.config {
        setup::run_setup();
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

    let mode = if args.alt {
        RequestMode::Alt
    } else if args.verbose {
        RequestMode::Verbose
    } else {
        RequestMode::Standard
    };

    let context = Context::detect();
    let config = Config::load();

    let res = RequestClient::new(args.input.clone(), context, config)
        .make_request(mode)
        .await;

    match res {
        Ok(response) => {
            println!("{}", response);

            // If -x flag and response contains a code block, offer to execute
            if args.execute {
                if let Some(cmd) = extract_command(&response) {
                    print!("\n{} {}", "Execute:".yellow(), cmd.bright_green());
                    print!(" {} ", "[y/N]".yellow());
                    io::stdout().flush().unwrap();

                    let mut input = String::new();
                    io::stdin().lock().read_line(&mut input).unwrap();

                    if input.trim().eq_ignore_ascii_case("y") {
                        println!("{}", "---".dimmed());
                        execute_command(&cmd);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Error: {}", e).red());
            std::process::exit(1);
        }
    }
}

/// Extract a command from markdown code blocks in the response
fn extract_command(response: &str) -> Option<String> {
    let mut in_block = false;
    let mut command = String::new();

    for line in response.lines() {
        if line.starts_with("```") {
            if in_block {
                let trimmed = command.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
                command.clear();
                in_block = false;
            } else {
                in_block = true;
                command.clear();
            }
        } else if in_block {
            if !command.is_empty() {
                command.push('\n');
            }
            command.push_str(line);
        }
    }

    None
}

fn execute_command(cmd: &str) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    let status = Command::new(&shell)
        .arg("-c")
        .arg(cmd)
        .status();

    match status {
        Ok(s) if !s.success() => {
            if let Some(code) = s.code() {
                eprintln!("{}", format!("Command exited with code {}", code).red());
            }
        }
        Err(e) => eprintln!("{}", format!("Failed to execute: {}", e).red()),
        _ => {}
    }
}

const REPO_URL: &str = "https://github.com/michaelyang12/suss.git";
const CARGO_TOML_URL: &str = "https://raw.githubusercontent.com/michaelyang12/suss/master/Cargo.toml";
const LOCAL_VERSION: &str = env!("CARGO_PKG_VERSION");

fn get_remote_version() -> Option<String> {
    let output = Command::new("curl")
        .args(["-sL", CARGO_TOML_URL])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let content = String::from_utf8(output.stdout).ok()?;
    for line in content.lines() {
        if line.starts_with("version") {
            let version = line.split('=').nth(1)?.trim().trim_matches('"');
            return Some(version.to_string());
        }
    }
    None
}

fn upgrade() {
    println!("{}", "Checking for updates...".cyan());

    match get_remote_version() {
        Some(remote_version) if remote_version == LOCAL_VERSION => {
            println!("{}", format!("Already up to date (v{}).", LOCAL_VERSION).green());
            return;
        }
        Some(remote_version) => {
            println!("{}", format!("Upgrading from v{} to v{}...", LOCAL_VERSION, remote_version).cyan());
        }
        None => {
            println!("{}", "Could not check remote version, upgrading anyway...".yellow());
        }
    }

    let status = Command::new("cargo")
        .args(["install", "--git", REPO_URL, "--locked", "--force"])
        .status();

    match status {
        Ok(s) if s.success() => println!("{}", "Upgrade complete!".green()),
        Ok(_) => eprintln!("{}", "Upgrade failed".red()),
        Err(e) => eprintln!("{}", format!("Failed to run cargo: {}", e).red()),
    }
}
