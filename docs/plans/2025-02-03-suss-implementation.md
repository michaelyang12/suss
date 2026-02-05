# suss Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust CLI (`ss`) that takes error output via stdin pipe or positional argument, detects project context, and sends it to an LLM for diagnosis and fix suggestions.

**Architecture:** Mirrors knock's module structure — clap for args, enum-based provider dispatch, raw HTTP for Anthropic/Ollama, async-openai for OpenAI. Adds stdin pipe detection and project-type detection. No caching or history (unlike knock).

**Tech Stack:** Rust 2024 edition, clap, reqwest, tokio, async-openai, serde/serde_json, colored, dotenvy

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "suss"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "ss"
path = "src/main.rs"

[dependencies]
clap = { version = "4.5", features = ["derive"] }
dotenvy = "0.15"
reqwest = { version = "0.12", features = ["blocking"] }
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
colored = "2"
async-openai = { version = "0.31", features = ["responses"] }
serde_json = "1.0"
```

**Step 2: Create minimal main.rs**

```rust
fn main() {
    println!("suss v0.1.0");
}
```

**Step 3: Verify it builds**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles successfully, binary at `target/debug/ss`

**Step 4: Verify binary name**

Run: `cd /home/myang/Source/suss && cargo run`
Expected: Prints "suss v0.1.0"

**Step 5: Initialize git repo and commit**

```bash
cd /home/myang/Source/suss
git init
echo '/target' > .gitignore
git add Cargo.toml src/main.rs .gitignore
git commit -m "init: scaffold suss project"
```

---

### Task 2: CLI Args

**Files:**
- Create: `src/args.rs`
- Modify: `src/main.rs`

**Step 1: Create args.rs**

```rust
use clap::Parser;

/// Error diagnosis assistant — pipe errors or paste them for LLM-powered help
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Error message to diagnose
    #[arg(default_value = "")]
    pub input: String,

    /// Include deeper context, common causes, and docs
    #[arg(short, long)]
    pub verbose: bool,

    /// Show multiple fix approaches with trade-offs
    #[arg(short, long)]
    pub alt: bool,

    /// Execute the suggested fix command after confirmation
    #[arg(short = 'x', long)]
    pub execute: bool,

    /// Configure provider and model
    #[arg(long)]
    pub config: bool,

    /// Upgrade to latest version from git
    #[arg(long)]
    pub upgrade: bool,
}
```

**Step 2: Update main.rs with arg parsing and stdin detection**

```rust
mod args;

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
```

Note: We need the `atty` crate for stdin TTY detection. Add to Cargo.toml:

```toml
atty = "0.2"
```

**Step 3: Build and test with argument**

Run: `cd /home/myang/Source/suss && cargo run -- "error[E0382]: borrow of moved value"`
Expected: Prints the input and TODO message

**Step 4: Test with pipe**

Run: `echo "error: something went wrong" | cargo run --`
Expected: Reads from stdin, prints input and TODO message

**Step 5: Test with no input**

Run: `cd /home/myang/Source/suss && cargo run`
Expected: Prints usage hint and exits with code 1

**Step 6: Commit**

```bash
git add src/args.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: add CLI arg parsing with stdin pipe detection"
```

---

### Task 3: Config & Provider

**Files:**
- Create: `src/config.rs`

**Step 1: Create config.rs**

Directly adapted from knock's config.rs — changed `.knock` to `.suss`:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    OpenAI,
    Anthropic,
    Ollama,
}

impl Default for Provider {
    fn default() -> Self {
        Provider::OpenAI
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub provider: Provider,
    pub openai_model: Option<String>,
    pub anthropic_model: Option<String>,
    pub ollama_model: Option<String>,
    pub ollama_url: Option<String>,
}

impl Config {
    fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".suss").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(e) if e.kind() == ErrorKind::NotFound => Self::default(),
            Err(_) => Self::default(),
        }
    }

    pub fn openai_model(&self) -> &str {
        self.openai_model.as_deref().unwrap_or("gpt-4o-mini")
    }

    pub fn anthropic_model(&self) -> &str {
        self.anthropic_model.as_deref().unwrap_or("claude-sonnet-4-20250514")
    }

    pub fn ollama_model(&self) -> &str {
        self.ollama_model.as_deref().unwrap_or("llama3.2")
    }

    pub fn ollama_url(&self) -> &str {
        self.ollama_url.as_deref().unwrap_or("http://localhost:11434")
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).unwrap();
        std::fs::write(path, json)
    }
}
```

**Step 2: Add `mod config;` to main.rs**

Add the module declaration to main.rs.

**Step 3: Build to verify**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add config module with provider enum and persistence"
```

---

### Task 4: Context Detection (Shell + Project)

**Files:**
- Create: `src/context.rs`

**Step 1: Create context.rs with shell detection (from knock) + project detection (new)**

```rust
use std::env;
use std::path::Path;
use std::process::Command;

pub struct Context {
    pub os: String,
    pub shell: String,
    pub cwd: String,
    pub project: Option<String>,
}

impl Context {
    pub fn detect() -> Self {
        let cwd = env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Self {
            os: detect_os(),
            shell: detect_shell(),
            project: detect_project(&cwd),
            cwd,
        }
    }

    pub fn as_prompt_context(&self) -> String {
        let project_tag = match &self.project {
            Some(p) => format!("\n  <project>{}</project>", p),
            None => String::new(),
        };
        format!(
            "<context>\n  <os>{}</os>\n  <shell>{}</shell>\n  <cwd>{}</cwd>{}\n</context>",
            self.os, self.shell, self.cwd, project_tag
        )
    }
}

fn detect_os() -> String {
    if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn detect_shell() -> String {
    if let Ok(shell_path) = env::var("SHELL") {
        if let Some(shell_name) = shell_path.split('/').last() {
            return shell_name.to_string();
        }
    }

    if cfg!(target_os = "windows") {
        if env::var("PSModulePath").is_ok() {
            return "powershell".to_string();
        }
        return "cmd".to_string();
    }

    if let Ok(output) = Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "ppid="])
        .output()
    {
        if let Ok(ppid) = String::from_utf8_lossy(&output.stdout).trim().parse::<u32>() {
            if let Ok(output) = Command::new("ps")
                .args(["-p", &ppid.to_string(), "-o", "comm="])
                .output()
            {
                let comm = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !comm.is_empty() {
                    return comm.split('/').last().unwrap_or(&comm).to_string();
                }
            }
        }
    }

    "unknown".to_string()
}

/// Walk up from cwd looking for project indicator files
fn detect_project(cwd: &str) -> Option<String> {
    let mut dir = Path::new(cwd);

    loop {
        // Check in priority order
        if dir.join("Cargo.toml").exists() {
            return Some("rust".to_string());
        }
        if dir.join("package.json").exists() {
            // Check if TypeScript
            if dir.join("tsconfig.json").exists() {
                return Some("typescript".to_string());
            }
            return Some("javascript".to_string());
        }
        if dir.join("go.mod").exists() {
            return Some("go".to_string());
        }
        if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
            return Some("python".to_string());
        }
        if dir.join("pom.xml").exists() {
            return Some("java".to_string());
        }
        if dir.join("build.gradle").exists() || dir.join("build.gradle.kts").exists() {
            return Some("java/kotlin".to_string());
        }

        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }

    None
}
```

**Step 2: Add `mod context;` to main.rs**

**Step 3: Build to verify**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/context.rs src/main.rs
git commit -m "feat: add context detection with shell info and project type"
```

---

### Task 5: LLM Client

**Files:**
- Create: `src/client.rs`

**Step 1: Create client.rs with system instructions and provider implementations**

```rust
use crate::config::{Config, Provider};
use crate::context::Context;
use async_openai::{
    Client, config::OpenAIConfig, error::OpenAIError, types::responses::CreateResponseArgs,
};
use serde::{Deserialize, Serialize};

const INSTRUCTIONS: &str = r#"
<system_instructions>
  <role>
    You are an error diagnosis assistant. You receive error output from CLI tools, compilers, and runtimes, along with context about the user's OS, shell, and project type. Your job is to explain the error and suggest a fix.
  </role>

  <output_format>
    Start with a brief explanation of the error (2-3 sentences): what went wrong and why.
    Then provide a concrete fix. If the fix is a command, format it as a code block.
    If the fix is a code change, show the relevant snippet.
  </output_format>

  <modes>
    <mode name="standard" default="true">
      Concise explanation + one concrete fix. No preamble, no fluff.
    </mode>

    <mode name="verbose">
      When [verbose] flag is present:
      1. Detailed explanation of the error
      2. Common causes for this error
      3. The recommended fix with explanation
      4. Links to relevant documentation if applicable
    </mode>

    <mode name="alt">
      When [alt] flag is present:
      PRIMARY FIX: The most common/recommended fix
      ALTERNATIVE 1: A different approach with trade-offs
      ALTERNATIVE 2: Another approach with trade-offs
    </mode>
  </modes>

  <constraints>
    Be specific to the detected project type and language.
    Don't repeat the error back to the user.
    Don't ask clarifying questions — make reasonable assumptions.
    If the error references a specific file/line, mention it in the fix.
    If you can't determine the cause, say so honestly and suggest debugging steps.
  </constraints>
</system_instructions>
"#;

pub struct RequestClient {
    input: String,
    context: Context,
    config: Config,
}

#[derive(Clone, Copy)]
pub enum RequestMode {
    Standard,
    Verbose,
    Alt,
}

impl RequestClient {
    pub fn new(input: String, context: Context, config: Config) -> Self {
        Self { input, context, config }
    }

    fn gen_prompt(&self, mode: RequestMode) -> String {
        let mode_tag = match mode {
            RequestMode::Standard => "",
            RequestMode::Verbose => " [verbose]",
            RequestMode::Alt => " [alt]",
        };
        format!(
            "{}\n\n<error_output>{}</error_output>{}",
            self.context.as_prompt_context(),
            self.input,
            mode_tag
        )
    }

    fn get_max_tokens(mode: RequestMode) -> u32 {
        match mode {
            RequestMode::Standard => 512,
            RequestMode::Verbose | RequestMode::Alt => 1024,
        }
    }

    pub async fn make_request(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match self.config.provider {
            Provider::OpenAI => self.request_openai(mode).await,
            Provider::Anthropic => self.request_anthropic(mode).await,
            Provider::Ollama => self.request_ollama(mode).await,
        }
    }

    async fn request_openai(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let client: Client<OpenAIConfig> = Client::new();
        let prompt = self.gen_prompt(mode);
        let request = CreateResponseArgs::default()
            .model(self.config.openai_model())
            .instructions(INSTRUCTIONS)
            .input(prompt)
            .temperature(0.2)
            .max_output_tokens(Self::get_max_tokens(mode))
            .build()?;

        let response = client.responses().create(request).await?;

        if let Some(text) = response.output_text() {
            Ok(text.clone())
        } else {
            Err(OpenAIError::InvalidArgument("Empty response".to_string()).into())
        }
    }

    async fn request_anthropic(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY not set")?;

        let prompt = self.gen_prompt(mode);

        let request_body = AnthropicRequest {
            model: self.config.anthropic_model().to_string(),
            max_tokens: Self::get_max_tokens(mode),
            system: INSTRUCTIONS.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt,
            }],
        };

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Anthropic API error: {}", error_text).into());
        }

        let response_body: AnthropicResponse = response.json().await?;

        response_body
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "Empty response from Anthropic".into())
    }

    async fn request_ollama(&self, mode: RequestMode) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let prompt = self.gen_prompt(mode);
        let url = format!("{}/api/chat", self.config.ollama_url());

        let request_body = OllamaRequest {
            model: self.config.ollama_model().to_string(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: INSTRUCTIONS.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            stream: false,
            options: OllamaOptions {
                temperature: 0.2,
                num_predict: Self::get_max_tokens(mode),
            },
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Ollama error: {}. Is Ollama running?", error_text).into());
        }

        let response_body: OllamaResponse = response.json().await?;
        Ok(response_body.message.content)
    }
}

// --- Anthropic types ---

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

// --- Ollama types ---

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}
```

**Step 2: Add `mod client;` to main.rs**

**Step 3: Build to verify**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles (warnings about unused imports are fine)

**Step 4: Commit**

```bash
git add src/client.rs src/main.rs
git commit -m "feat: add LLM client with OpenAI, Anthropic, and Ollama providers"
```

---

### Task 6: Setup Wizard

**Files:**
- Create: `src/setup.rs`

**Step 1: Create setup.rs**

Adapted from knock's setup.rs — changed branding to suss:

```rust
use crate::config::{Config, Provider};
use colored::*;
use std::io::{self, BufRead, Write};
use std::process::Command;

const OPENAI_MODELS: &[&str] = &[
    "gpt-4o-mini",
    "gpt-4o",
    "o1-mini",
];

const ANTHROPIC_MODELS: &[&str] = &[
    "claude-sonnet-4-20250514",
    "claude-opus-4-20250514",
    "claude-3-5-haiku-20241022",
];

pub fn run_setup() {
    println!("{}", "suss configuration\n".bold());

    let current = Config::load();
    let current_provider = match current.provider {
        Provider::OpenAI => "openai",
        Provider::Anthropic => "anthropic",
        Provider::Ollama => "ollama",
    };
    let current_model = match current.provider {
        Provider::OpenAI => current.openai_model(),
        Provider::Anthropic => current.anthropic_model(),
        Provider::Ollama => current.ollama_model(),
    };
    println!("Current: {} / {}\n", current_provider.cyan(), current_model.cyan());

    println!("Select provider:");
    let providers = [
        ("OpenAI", Provider::OpenAI),
        ("Anthropic", Provider::Anthropic),
        ("Ollama (local)", Provider::Ollama),
    ];
    for (i, (name, p)) in providers.iter().enumerate() {
        let marker = if std::mem::discriminant(p) == std::mem::discriminant(&current.provider) {
            " ←".cyan().to_string()
        } else {
            String::new()
        };
        println!("  {}. {}{}", i + 1, name, marker);
    }
    print!("\n> ");
    io::stdout().flush().unwrap();

    let provider_choice = read_line();
    let provider = match provider_choice.trim() {
        "1" => Provider::OpenAI,
        "2" => Provider::Anthropic,
        "3" => Provider::Ollama,
        _ => {
            eprintln!("{}", "Invalid choice".red());
            return;
        }
    };

    match &provider {
        Provider::OpenAI => {
            if std::env::var("OPENAI_API_KEY").is_err() {
                eprintln!("\n{}", "Warning: OPENAI_API_KEY not set".yellow());
                eprintln!("Add to your shell profile:");
                eprintln!("  export OPENAI_API_KEY=\"your_key_here\"\n");
            } else {
                println!("\n{}", "OPENAI_API_KEY found".green());
            }
        }
        Provider::Anthropic => {
            if std::env::var("ANTHROPIC_API_KEY").is_err() {
                eprintln!("\n{}", "Warning: ANTHROPIC_API_KEY not set".yellow());
                eprintln!("Add to your shell profile:");
                eprintln!("  export ANTHROPIC_API_KEY=\"your_key_here\"\n");
            } else {
                println!("\n{}", "ANTHROPIC_API_KEY found".green());
            }
        }
        Provider::Ollama => {
            if !check_ollama_running() {
                eprintln!("\n{}", "Warning: Ollama doesn't appear to be running".yellow());
                eprintln!("Start it with: ollama serve\n");
            } else {
                println!("\n{}", "Ollama is running".green());
            }
        }
    }

    let model = select_model(&provider, &current);

    let mut config = Config::default();
    config.provider = provider.clone();

    match provider {
        Provider::OpenAI => config.openai_model = model,
        Provider::Anthropic => config.anthropic_model = model,
        Provider::Ollama => config.ollama_model = model,
    }

    if let Err(e) = config.save() {
        eprintln!("{}", format!("Failed to save config: {}", e).red());
        return;
    }

    println!("\n{}", "Configuration saved to ~/.suss/config.json".green());
}

fn select_model(provider: &Provider, current_config: &Config) -> Option<String> {
    let models: Vec<String> = match provider {
        Provider::OpenAI => OPENAI_MODELS.iter().map(|s| s.to_string()).collect(),
        Provider::Anthropic => ANTHROPIC_MODELS.iter().map(|s| s.to_string()).collect(),
        Provider::Ollama => {
            print!("Fetching models...");
            io::stdout().flush().unwrap();
            let m = get_ollama_models();
            print!("\r                   \r");
            m
        }
    };

    if models.is_empty() {
        eprintln!("{}", "No models available".yellow());
        return None;
    }

    let current_model = match provider {
        Provider::OpenAI => current_config.openai_model(),
        Provider::Anthropic => current_config.anthropic_model(),
        Provider::Ollama => current_config.ollama_model(),
    };

    println!("\nSelect model:");
    for (i, model) in models.iter().enumerate() {
        let marker = if model == current_model {
            " ←".cyan().to_string()
        } else if i == 0 {
            " (default)".dimmed().to_string()
        } else {
            String::new()
        };
        println!("  {}. {}{}", i + 1, model, marker);
    }
    print!("\n> ");
    io::stdout().flush().unwrap();

    let choice = read_line();
    let choice = choice.trim();

    if choice.is_empty() {
        return None;
    }

    match choice.parse::<usize>() {
        Ok(n) if n >= 1 && n <= models.len() => Some(models[n - 1].clone()),
        _ => {
            eprintln!("{}", "Invalid choice, using default".yellow());
            None
        }
    }
}

fn get_ollama_models() -> Vec<String> {
    let output = Command::new("ollama").arg("list").output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .skip(1)
                .filter_map(|line| line.split_whitespace().next().map(|s| s.to_string()))
                .collect()
        }
        _ => vec![],
    }
}

fn check_ollama_running() -> bool {
    reqwest::blocking::get("http://localhost:11434/api/tags").is_ok()
}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input).unwrap();
    input
}
```

**Step 2: Add `mod setup;` to main.rs, wire up `args.config` to `setup::run_setup()`**

**Step 3: Build to verify**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/setup.rs src/main.rs
git commit -m "feat: add interactive setup wizard for provider and model config"
```

---

### Task 7: Wire Up Main — Full Flow

**Files:**
- Modify: `src/main.rs`

**Step 1: Write the complete main.rs orchestration**

```rust
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
```

**Step 2: Build and test full flow**

Run: `cd /home/myang/Source/suss && cargo build`
Expected: Compiles successfully

**Step 3: Test with a real error (requires an API key set)**

Run: `echo "error[E0382]: borrow of moved value: \`x\`" | cargo run --`
Expected: LLM response with explanation and fix

**Step 4: Test verbose mode**

Run: `echo "error[E0382]: borrow of moved value" | cargo run -- -v`
Expected: Detailed response with common causes

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up full flow — stdin/arg input, context, LLM request, execute"
```

---

### Task 8: Polish & README

**Files:**
- Create: `README.md`

**Step 1: Create README.md**

```markdown
# suss

Error diagnosis assistant. Pipe errors or paste them for LLM-powered explanations and fixes.

## Install

```bash
cargo install --git https://github.com/michaelyang12/suss.git --locked
```

## Usage

```bash
# Pipe errors
cargo build 2>&1 | ss
npm run build 2>&1 | ss

# Paste directly
ss "error[E0382]: borrow of moved value"

# Verbose — deeper context and docs
cargo build 2>&1 | ss -v

# Alternatives — multiple fix approaches
cargo build 2>&1 | ss --alt

# Execute — run the suggested fix
cargo build 2>&1 | ss -x
```

## Setup

```bash
ss --config
```

Supports OpenAI, Anthropic, and Ollama (local). API keys via environment variables:

```bash
export OPENAI_API_KEY="..."
export ANTHROPIC_API_KEY="..."
```

## How it works

1. Reads error output from stdin (pipe) or positional argument
2. Detects your OS, shell, and project type (Rust, JS/TS, Go, Python, Java, etc.)
3. Sends context + error to your configured LLM
4. Returns a concise explanation and fix
```

**Step 2: Final build + test**

Run: `cd /home/myang/Source/suss && cargo build --release`
Expected: Release binary at `target/release/ss`

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add README with install and usage instructions"
```
