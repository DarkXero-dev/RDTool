use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::client::RdClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RdUser {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub points: u64,
    pub locale: String,
    pub avatar: Option<String>,
    #[serde(rename = "type")]
    pub account_type: String,
    pub premium: u64,
    pub expiration: Option<String>,
}

pub async fn get_user(client: &RdClient) -> Result<RdUser> {
    let resp = client.get("/user").send().await?;
    let user: RdUser = resp.error_for_status()?.json().await?;
    Ok(user)
}
