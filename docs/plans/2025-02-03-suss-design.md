# suss — Error Helper CLI

## Overview

CLI tool that takes error output (piped or pasted) and sends it to an LLM for explanation and suggested fixes. Like a reverse `knock` — instead of translating intent → commands, it translates errors → understanding.

**Binary name:** `ss`

## Input Flow

- **Pipe:** `cargo build 2>&1 | ss`
- **Argument:** `ss "error[E0382]: borrow of moved value"`
- **No input:** Print usage hint and exit
- **Detection:** Check if stdin is a TTY. If not, read from pipe. If TTY, expect positional arg.

## LLM Providers

Reuse knock's provider pattern exactly:

- OpenAI (env: `OPENAI_API_KEY`, default model: `gpt-4o-mini`)
- Anthropic (env: `ANTHROPIC_API_KEY`, default model: `claude-sonnet-4-20250514`)
- Ollama (default: `llama3.2` at `http://localhost:11434`)

Config stored at `~/.suss/config.json`. Interactive setup via `ss --config`.

## Modes

| Flag | Mode | Behavior | Max Tokens |
|------|------|----------|------------|
| (none) | Default | Explanation + fix | 512 |
| `-v` | Verbose | Deeper context, common causes, docs | 1024 |
| `--alt` | Alternatives | Multiple fix approaches with trade-offs | 1024 |
| `-x` | Execute | Offer to run fix command with confirmation | 512 |
| `--config` | Setup | Interactive provider/model wizard | — |
| `--upgrade` | Upgrade | Self-update via cargo install | — |

**Temperature:** 0.2

**No caching or history** — error messages are rarely identical enough to benefit.

## Project Detection

Walk up from cwd looking for known project files:

| File | Language |
|------|----------|
| Cargo.toml | Rust |
| package.json | Node/JS/TS |
| go.mod | Go |
| pyproject.toml | Python |
| requirements.txt | Python |
| pom.xml | Java |
| build.gradle | Java/Kotlin |
| *.sln | C#/.NET |

Context sent as XML alongside shell info:

```xml
<context>
  <os>Linux</os>
  <shell>bash</shell>
  <cwd>/home/myang/Source/suss</cwd>
  <project>rust</project>
</context>
```

## System Instructions

Embedded in client.rs. Tells the LLM:

- Role: error diagnosis assistant
- Receives error output + shell/project context
- Default: 2-3 sentence explanation + concrete fix
- Verbose: deeper context, common causes, relevant docs
- Alt: 2-3 alternative approaches with trade-offs
- Be specific to detected language/framework
- Format runnable fixes as code blocks

## File Structure

```
suss/
├── Cargo.toml
├── src/
│   ├── main.rs      # Entry point, stdin/arg handling, orchestration
│   ├── args.rs      # Clap definitions (flags, modes)
│   ├── config.rs    # Config struct, Provider enum, load/save
│   ├── client.rs    # RequestClient, provider implementations, instructions
│   ├── context.rs   # ShellContext + ProjectDetection
│   └── setup.rs     # Interactive config wizard
└── README.md
```
