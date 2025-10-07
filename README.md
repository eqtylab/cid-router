# CID Router

<hr/>
Content address everything.
<hr/>

`cid-router` resolves requests for [CIDs](https://docs.ipfs.tech/concepts/content-addressing/#what-is-a-cid) into [routes](/crates/routes) for retrieval of the content and/or resources related to the content.

`cid-router` is not opinionated about types of CIDs, routes, or underlying protocols used by routes.

`cid-router` can be configured to consume routes from any number of CRPs (CID Route Provider).

<div align="center">
  <img src="/.readme/cid-router.svg" alt="CID Router diagram">
</div>

# Repo Organization
|||
|-|-|
|[core](/core)|CID Router Core (Rust lib crate) |
|[crates](/crates)| |
|&emsp;[api-utils](/crates/api-utils)|Utility library for API binaries |
|[crps](/crps)|Crates for individual CID Route Providers |
|&emsp;[azure](/crps/azure)| |
|&emsp;[iroh](/crps/iroh)|Iroh CRP Crate |
|[server](/server)|CID Router Server |

# Quick Start

Nix is used for managing development environments and build artifacts.

Enter development shell with all required tools:

```shell
nix develop
```

# Makefile

There is a Makefile to make the nix-managed builds and docker exports simpler to manage. Build artifacts are placed in `_build/`

Build `cid-router` binary:

```shell
make bin.cid-router
```

Build `cid-router` docker image:

```shell
make image.cid-router`
```

Build all docker images:

```shell
make image.all
```

Build all artifacts:

```shell
make all
```

# Justfile
```present just
Available recipes:
    ci            # Run main CI job
    readme-check  # Check auto-generated portions of README.md
    readme-update # Update auto-generated portions of README.md
```

# Installing Nix

Install

    https://nixos.org/download.html

Configure for using flakes

```shell
sudo sh -c 'echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf'
```
