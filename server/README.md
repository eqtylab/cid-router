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

Usage: cid-router-server start --config <CONFIG>

Options:
  -c, --config <CONFIG>  Config file to use
  -h, --help             Print help
```

# Example Config

```present cat config.example.toml
port = 3080

# [[providers]]
# type = "ipfs"
# gateway_url = "http://localhost:8080"

[[providers]]
type = "iroh"
# node_addr_ref = { node_id = "w36hbmld67hrocfllnfca4ahzae2ibrom2moj2lovjguye3gkmiq" }
node_addr_ref = { ticket = "blobaccbd3d6iyowiix4ixt5btbxndo5mamzbhcbfksn55krurogsrgbwajdnb2hi4dthixs65ltmuys2mjoojswyylzfzuxe33ifzxgk5dxn5zgwlrpauaesa732pf6aaqavqiqaaol4abablataaa4xyacacwboaabzpqaeagavaafbs7aaiax3vlpwtrmwr4owttczv6g4pglwz26xxj4bgovjfcmvus7awi6dda" }
```
