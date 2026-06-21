use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::client::RdClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct StreamInfo {
    pub apple: Option<HashMap<String, String>>,
    pub dash: Option<HashMap<String, String>>,
    pub liveMP4: Option<HashMap<String, String>>,
    pub h264WebM: Option<HashMap<String, String>>,
}

pub async fn get_stream_transcodes(client: &RdClient, id: &str) -> Result<StreamInfo> {
    let resp = client
        .get(&format!("/streaming/transcode/{id}"))
        .send()
        .await?;
    let info: StreamInfo = resp.error_for_status()?.json().await?;
    Ok(info)
}
