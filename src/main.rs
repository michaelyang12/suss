mod args;
mod context;

use std::io::{self, BufRead, Read, Write};
use std::process::Command;

use args::Args;
use conduit::{Config, RequestOptions};
use context::Context;
use clap::Parser;
use colored::*;

const INSTRUCTIONS: &str = r#"
<system_instructions>
  <role>
    You are an error diagnosis assistant. You receive error output from CLI tools, compilers, and runtimes, along with context about the user's OS, shell, and project type.
  </role>

  <output_format>
    You MUST use this exact format. No deviations.

    CAUSE: One sentence explaining what went wrong and why.

    FIX: The concrete fix — either a command or code snippet. No explanation here, just the fix itself. If it's a command, write it on its own line with no backticks or formatting. If it's a code change, show only the minimal relevant lines.

    That's it. Two sections. Nothing else in standard mode.
  </output_format>

  <modes>
    <mode name="standard" default="true">
      Exactly CAUSE + FIX as described above. Maximum 5 lines total.
    </mode>

    <mode name="verbose">
      When [verbose] flag is present, use this format:

      CAUSE: One sentence.

      WHY: 2-3 sentences with deeper context on why this happens.

      COMMON CAUSES:
      - First common cause
      - Second common cause
      - Third common cause

      FIX: The concrete fix.

      DOCS: One relevant documentation link if applicable. Omit if none.
    </mode>

    <mode name="alt">
      When [alt] flag is present, use this format:

      CAUSE: One sentence.

      FIX 1: The recommended fix.

      FIX 2: An alternative approach.
      TRADE-OFF: One sentence on when to prefer this.

      FIX 3: Another alternative.
      TRADE-OFF: One sentence on when to prefer this.
    </mode>
  </modes>

  <constraints>
    STRICT RULES — violating these is a failure:
    - Use ONLY the section headers specified above (CAUSE, FIX, WHY, etc). No other headers.
    - NO markdown formatting. No backticks, no bold, no bullet points except where specified.
    - NO preamble, greeting, or sign-off.
    - NO repeating the error back to the user.
    - NO "you can also try" or "another option" in standard mode.
    - NO asking clarifying questions.
    - Be specific to the detected project type and language.
    - If a file/line is referenced in the error, mention it in the fix.
    - If you genuinely can't determine the cause, say so in CAUSE and suggest a debugging step in FIX.
    - Keep it terse. Every word must earn its place.
  </constraints>
</system_instructions>
"#;

#[derive(Clone, Copy)]
enum RequestMode {
    Standard,
    Verbose,
    Alt,
}

#[tokio::main]
async fn main() {
    let mut args = Args::parse();

    if args.upgrade {
        upgrade();
        return;
    }

    if args.config {
        conduit::setup::run_setup("suss", None);
        return;
    }

    // Read from stdin if piped
    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_input = String::new();
        io::stdin().read_to_string(&mut stdin_input).unwrap();
        args.input = stdin_input;
    }

    if args.input.trim().is_empty() {
        eprintln!("{}", "Usage: suss \"error message\" or pipe: cmd 2>&1 | suss".red());
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
    let config = Config::load("suss");

    // Build prompt
    let mode_tag = match mode {
        RequestMode::Standard => "",
        RequestMode::Verbose => " [verbose]",
        RequestMode::Alt => " [alt]",
    };
    let prompt = format!(
        "{}\n\n<error_output>{}</error_output>{}",
        context.as_prompt_context(),
        args.input,
        mode_tag
    );

    let max_tokens = match mode {
        RequestMode::Standard => 512,
        RequestMode::Verbose | RequestMode::Alt => 1024,
    };

    let opts = RequestOptions {
        max_tokens,
        ..Default::default()
    };

    let res = conduit::request(&config, INSTRUCTIONS, &prompt, opts).await;

    match res {
        Ok(response) => {
            print_formatted(&response);

            // If -x flag, extract fix command and offer to execute
            if args.execute {
                if let Some(cmd) = extract_fix_command(&response) {
                    print!("\n{} {}", "Run:".yellow(), cmd.bright_green());
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

/// Format and colorize the LLM response
fn print_formatted(response: &str) {
    let headers = ["CAUSE:", "FIX:", "FIX 1:", "FIX 2:", "FIX 3:",
                    "WHY:", "COMMON CAUSES:", "DOCS:", "TRADE-OFF:"];

    for line in response.lines() {
        let trimmed = line.trim();

        if headers.iter().any(|h| trimmed.starts_with(h)) {
            // Split into header and content
            if let Some(pos) = trimmed.find(':') {
                let (header, rest) = trimmed.split_at(pos + 1);
                print!("{}", header.cyan().bold());
                println!("{}", rest);
            }
        } else if trimmed.starts_with("- ") {
            // Bullet points in COMMON CAUSES
            println!("  {}", trimmed.dimmed());
        } else if trimmed.is_empty() {
            println!();
        } else {
            println!("  {}", trimmed);
        }
    }
}

/// Extract the first FIX command from the response (for -x execute mode)
fn extract_fix_command(response: &str) -> Option<String> {
    let mut in_fix = false;
    let mut command = String::new();

    for line in response.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("FIX:") || trimmed.starts_with("FIX 1:") {
            let content = trimmed
                .trim_start_matches("FIX 1:")
                .trim_start_matches("FIX:")
                .trim();
            if !content.is_empty() {
                return Some(content.to_string());
            }
            in_fix = true;
        } else if in_fix {
            if trimmed.is_empty() || trimmed.ends_with(':') {
                break;
            }
            if !command.is_empty() {
                command.push('\n');
            }
            command.push_str(trimmed);
        }
    }

    if command.trim().is_empty() { None } else { Some(command.trim().to_string()) }
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
