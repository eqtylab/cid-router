# Overview

CID Router Server

# Usage

```present cargo run -- --help
cid-router

Usage: cid-router-server <COMMAND>

Commands:
  start    Start service
  openapi  Generate OpenAPI json documents
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `cid-router start`

```present cargo run -- start --help
Start service

Usage: cid-router-server start [OPTIONS]

Options:
  -r, --repo-path <REPO_PATH>  Repo Path to use to use
  -h, --help                   Print help
```

# Example Config

```present cat config.example.toml
port = 3080
auth = "none"

[[providers]]
type = "iroh"
path = "/Users/rklaehn/projects_git/cid-router/blobs"
```
