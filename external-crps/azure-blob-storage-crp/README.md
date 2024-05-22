# Overview

Azure Blob Storage CRP Service

# Usage

```present cargo run -- --help
azure-blob-storage-crp

Usage: azure-blob-storage-crp <COMMAND>

Commands:
  start  Start service
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `azure-blob-storage-crp start`

```present cargo run -- start --help
Start service

Usage: azure-blob-storage-crp start --config <CONFIG>

Options:
  -c, --config <CONFIG>  Config file to use
  -h, --help             Print help
```

# Example Config

```present cat config.example.toml
port = 3081

indexing_strategy = { poll_interval = 60 }

db_file = "./db.redb"

log_level_default = "error"
log_level_app = "trace"

[[blob_storage.containers]]
account = "cameronsa1"
container = "blobstorage1"
filter = "all"

[[blob_storage.containers]]
account = "shareddatastgacct"
container = "shared-data"
filter = { and = [
    { not = { file_ext = "pdf" } },
    { or = [
        { file_ext = "csv" },
        # { file_ext = "json" },
    ]},
    { size = { max = 10_000_000 } }
]}
```
