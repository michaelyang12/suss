# suss

Error diagnosis assistant. Pipe errors or paste them for LLM-powered explanations and fixes.

## Install

```bash
cargo install --git https://github.com/michaelyang12/suss.git --locked
```

## Usage

```bash
# Pipe errors
cargo build 2>&1 | suss
npm run build 2>&1 | suss

# Paste directly
suss "error[E0382]: borrow of moved value"

# Verbose — deeper context and docs
cargo build 2>&1 | suss -v

# Alternatives — multiple fix approaches
cargo build 2>&1 | suss --alt

# Execute — run the suggested fix
cargo build 2>&1 | suss -x
```

## Setup

```bash
suss --config
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
