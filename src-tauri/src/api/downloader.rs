use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::client::RdClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct UnrestrictedLink {
    pub id: String,
    pub filename: String,
    pub mimeType: Option<String>,
    pub filesize: u64,
    pub link: String,
    pub host: String,
    pub download: String,
    pub streamable: Option<u8>,
}

pub async fn unrestrict_link(client: &RdClient, link: &str) -> Result<UnrestrictedLink> {
    let resp = client
        .post("/unrestrict/link")
        .form(&[("link", link)])
        .send()
        .await?;
    let result: UnrestrictedLink = resp.error_for_status()?.json().await?;
    Ok(result)
}

pub async fn unrestrict_links(
    client: &RdClient,
    links: Vec<String>,
) -> Result<Vec<UnrestrictedLink>> {
    let mut results = Vec::new();
    for link in links {
        match unrestrict_link(client, &link).await {
            Ok(r) => results.push(r),
            Err(e) => eprintln!("failed to unrestrict {link}: {e}"),
        }
    }
    Ok(results)
}
