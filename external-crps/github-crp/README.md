# Overview

Github CRP Service

# Usage

```present cargo run -- --help
github-crp

Usage: github-crp <COMMAND>

Commands:
  start  Start service
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `github-crp start`

```present cargo run -- start --help
Start service

Usage: github-crp start --config <CONFIG>

Options:
  -c, --config <CONFIG>  Config file to use
  -h, --help             Print help
```

# Example Config

```present cat config.example.toml
port = 3082

indexing_strategy = { poll_interval = 3600 }

db_file = "./db.redb"

log_level_default = "error"
log_level_app = "trace"

[[repos]]
and = [
    { owned_by = "eqtylab" },
    { not.repo = { owner = "eqtylab", repo = "web-verifier-plus" } },
]

[[repos]]
repo = { owner = "n0-computer", repo = "iroh" }
```
