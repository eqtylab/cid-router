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
|[cid-router](/cid-router)|CID Router Service |
|[crates](/crates)| |
|&emsp;[api-utils](/crates/api-utils)|Utility library for API binaries |
|&emsp;[cid-filter](/crates/cid-filter)|CID filter model |
|&emsp;[routes](/crates/routes)|Routes model |
|[external-crps](/external-crps)| |
|&emsp;[azure-blob-storage-crp](/external-crps/azure-blob-storage-crp)|Azure Blob Storage CRP |
|&emsp;[github-crp](/external-crps/github-crp)|Github CRP |
 
# Justfile
```present just
Available recipes:
    ci            # Run main CI job
    readme-check  # Check auto-generated portions of README.md
    readme-update # Update auto-generated portions of README.md
```