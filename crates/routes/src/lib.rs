use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A route defining a method for resolving a CID to its content and/or metadata associated with its content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Route {
    /// CID Route Provider ID.
    /// This optional value is only meant for use by CID Routers with multiple CID Route Providers.
    /// CID Route providers themselves don't need to set it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crp_id: Option<String>,
    /// Type of the route.
    #[serde(rename = "type")]
    pub type_: String,
    /// Method for resolving a CID.
    /// Schema for the `method` is defined by the `type` field.
    pub method: Value,
    /// Metadata for the route.
    /// Schema for the `metadata` is defined by the `type` field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

pub trait IntoRoute: Sized + Serialize {
    fn type_str() -> &'static str;

    fn into_route(
        self,
        crp_id: Option<String>,
        metadata: Option<Value>,
    ) -> Result<Route, serde_json::Error> {
        Ok(Route {
            crp_id,
            type_: Self::type_str().to_owned(),
            method: serde_json::to_value(self)?,
            metadata,
        })
    }
}

/// URL Route Method
///
/// Resolve a CID by fetching content from a URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlRouteMethod {
    /// URL
    pub url: String,
}

impl IntoRoute for UrlRouteMethod {
    fn type_str() -> &'static str {
        "url"
    }
}

/// IPFS Route Method
///
/// Resolve a CID by fetching content from the global IPFS network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpfsRouteMethod {
    /// CID
    pub cid: String,
}

impl IntoRoute for IpfsRouteMethod {
    fn type_str() -> &'static str {
        "ipfs"
    }
}

/// Iroh Route Method
///
/// Resolve a CID by fetching content from an Iroh node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrohRouteMethod {
    /// Ticket
    pub ticket: String,
}

impl IntoRoute for IrohRouteMethod {
    fn type_str() -> &'static str {
        "iroh"
    }
}

/// Azure Blob Storage Route Method
///
/// Resolve a CID by fetching content from Azure Blob Storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureBlobStorageRouteMethod {
    /// Account
    pub account: String,
    /// Container
    pub container: String,
    /// Blob
    pub name: String,
}

impl IntoRoute for AzureBlobStorageRouteMethod {
    fn type_str() -> &'static str {
        "azure_blob_storage"
    }
}

/// AWS S3 Route Method
///
/// Resolve a CID by fetching content from an AWS S3 bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsS3RouteMethod {
    /// Bucket
    pub bucket: String,
    /// Key
    pub object: String,
}

impl IntoRoute for AwsS3RouteMethod {
    fn type_str() -> &'static str {
        "aws_s3"
    }
}

/// Github Commit Route Method
///
/// Resolve a CID by fetching content from Github.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRouteMethod {
    /// Owner
    pub owner: String,
    /// Repository
    pub repo: String,
    /// Ref
    #[serde(rename = "ref")]
    pub ref_: GithubRef,
    /// Path (optional path to a subdirectory or file in the repository)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Part of [`GithubRouteMethod`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubRef {
    Branch(String),
    Tag(String),
    Commit(String),
}

impl IntoRoute for GithubRouteMethod {
    fn type_str() -> &'static str {
        "github"
    }
}

/// HuggingFace Route Method
///
/// Resolve a CID by fetching content from HuggingFace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceRouteMethod {
    /// Repository
    pub repo: String,
    /// Ref
    #[serde(rename = "ref")]
    pub ref_: HuggingFaceRef,
    /// Path (optional path to a subdirectory or file in the repository)
    pub path: Option<String>,
}

/// Part of [`HuggingFaceRouteMethod`]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HuggingFaceRef {
    Branch(String),
    Tag(String),
    Commit(String),
}

impl IntoRoute for HuggingFaceRouteMethod {
    fn type_str() -> &'static str {
        "huggingface"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn url_route_method() {
        let url_route_method = UrlRouteMethod {
            url: "https://example.com".to_owned(),
        };

        let route = Route {
            crp_id: None,
            type_: "url".to_owned(),
            method: json!({
                "url": "https://example.com",
            }),
            metadata: None,
        };

        assert_eq!(route, url_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn ipfs_route_method() {
        let ipfs_route_method = IpfsRouteMethod {
            cid: "bafybeigmfwlweiecbubdw4lq6uqngsioqepntcfohvrccr2o5f7flgydme".to_owned(),
        };

        let route = Route {
            crp_id: None,
            type_: "ipfs".to_owned(),
            method: json!({
                "cid": "bafybeigmfwlweiecbubdw4lq6uqngsioqepntcfohvrccr2o5f7flgydme",
            }),
            metadata: None,
        };

        assert_eq!(route, ipfs_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn iroh_route_method() {
        let iroh_route_method = IrohRouteMethod {
            ticket: "blobaccbd3d6iyowiix4ixt5btbxndo5mamzbhcbfksn55krurogsrgbwajdnb2hi4dthixs65ltmuys2mjoojswyylzfzuxe33ifzxgk5dxn5zgwlrpauaesa732pf6aaqavqiqaaol4abablataaa4xyacacwboaabzpqaeagavaafbs7aaiax3vlpwtrmwr4owttczv6g4pglwz26xxj4bgovjfcmvus7awi6dda".to_owned(),
        };

        let route = Route {
            crp_id: None,
            type_: "iroh".to_owned(),
            method: json!({
                "ticket": "blobaccbd3d6iyowiix4ixt5btbxndo5mamzbhcbfksn55krurogsrgbwajdnb2hi4dthixs65ltmuys2mjoojswyylzfzuxe33ifzxgk5dxn5zgwlrpauaesa732pf6aaqavqiqaaol4abablataaa4xyacacwboaabzpqaeagavaafbs7aaiax3vlpwtrmwr4owttczv6g4pglwz26xxj4bgovjfcmvus7awi6dda",
            }),
            metadata: None,
        };

        assert_eq!(route, iroh_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn azure_blob_storage_route_method() {
        let azure_blob_storage_route_method = AzureBlobStorageRouteMethod {
            account: "account".to_owned(),
            container: "container".to_owned(),
            name: "name".to_owned(),
        };

        let route = Route {
            crp_id: None,
            type_: "azure_blob_storage".to_owned(),
            method: json!({
                "account": "account",
                "container": "container",
                "name": "name",
            }),
            metadata: None,
        };

        assert_eq!(
            route,
            azure_blob_storage_route_method
                .into_route(None, None)
                .unwrap()
        );
    }

    #[test]
    fn aws_s3_route_method() {
        let aws_s3_route_method = AwsS3RouteMethod {
            bucket: "bucket".to_owned(),
            object: "object".to_owned(),
        };

        let route = Route {
            crp_id: None,
            type_: "aws_s3".to_owned(),
            method: json!({
                "bucket": "bucket",
                "object": "object",
            }),
            metadata: None,
        };

        assert_eq!(route, aws_s3_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn github_route_method_branch() {
        let github_route_method = GithubRouteMethod {
            owner: "owner".to_owned(),
            repo: "repo".to_owned(),
            ref_: GithubRef::Branch("main".to_owned()),
            path: Some("path".to_owned()),
        };

        let route = Route {
            crp_id: None,
            type_: "github".to_owned(),
            method: json!({
                "owner": "owner",
                "repo": "repo",
                "ref": {
                    "branch": "main",
                },
                "path": "path",
            }),
            metadata: None,
        };

        assert_eq!(route, github_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn github_route_method_tag() {
        let github_route_method = GithubRouteMethod {
            owner: "owner".to_owned(),
            repo: "repo".to_owned(),
            ref_: GithubRef::Tag("v1.0.0".to_owned()),
            path: Some("path".to_owned()),
        };

        let route = Route {
            crp_id: None,
            type_: "github".to_owned(),
            method: json!({
                "owner": "owner",
                "repo": "repo",
                "ref": {
                    "tag": "v1.0.0",
                },
                "path": "path",
            }),
            metadata: None,
        };

        assert_eq!(route, github_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn github_route_method_commit() {
        let github_route_method = GithubRouteMethod {
            owner: "owner".to_owned(),
            repo: "repo".to_owned(),
            ref_: GithubRef::Commit("sha".to_owned()),
            path: Some("path".to_owned()),
        };

        let route = Route {
            crp_id: None,
            type_: "github".to_owned(),
            method: json!({
                "owner": "owner",
                "repo": "repo",
                "ref": {
                    "commit": "sha",
                },
                "path": "path",
            }),
            metadata: None,
        };

        assert_eq!(route, github_route_method.into_route(None, None).unwrap());
    }

    #[test]
    fn huggingface_route_method() {
        let huggingface_route_method = HuggingFaceRouteMethod {
            repo: "repo".to_owned(),
            ref_: HuggingFaceRef::Branch("main".to_owned()),
            path: Some("path".to_owned()),
        };

        let route = Route {
            crp_id: None,
            type_: "huggingface".to_owned(),
            method: json!({
                "repo": "repo",
                "ref": {
                    "branch": "main",
                },
                "path": "path",
            }),
            metadata: None,
        };

        assert_eq!(
            route,
            huggingface_route_method.into_route(None, None).unwrap()
        );
    }
}
