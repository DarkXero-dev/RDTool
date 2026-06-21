use anyhow::Result;
use reqwest::multipart;
use serde::{Deserialize, Serialize};

use super::client::RdClient;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TorrentAddResult {
    pub id: String,
    pub uri: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TorrentFile {
    pub id: u32,
    pub path: String,
    pub bytes: u64,
    pub selected: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Torrent {
    pub id: String,
    pub filename: String,
    pub hash: Option<String>,
    pub bytes: u64,
    pub links: Vec<String>,
    pub status: String,
    pub progress: f64,
    pub seeders: Option<u32>,
    pub speed: Option<u64>,
    pub added: String,
    pub files: Option<Vec<TorrentFile>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct RdDownload {
    pub id: String,
    pub filename: String,
    pub mimeType: Option<String>,
    pub filesize: u64,
    pub link: String,
    pub host: String,
    pub download: String,
    pub generated: String,
}

pub async fn add_magnet(client: &RdClient, magnet: &str) -> Result<TorrentAddResult> {
    let resp = client
        .post("/torrents/addMagnet")
        .form(&[("magnet", magnet)])
        .send()
        .await?;
    let result: TorrentAddResult = resp.error_for_status()?.json().await?;
    Ok(result)
}

pub async fn add_torrent_file(
    client: &RdClient,
    bytes: Vec<u8>,
    filename: String,
) -> Result<TorrentAddResult> {
    let part = multipart::Part::bytes(bytes).file_name(filename);
    let form = multipart::Form::new().part("torrent", part);
    let resp = client.put("/torrents/addTorrent").multipart(form).send().await?;
    let result: TorrentAddResult = resp.error_for_status()?.json().await?;
    Ok(result)
}

pub async fn get_torrents(client: &RdClient) -> Result<Vec<Torrent>> {
    let resp = client.get("/torrents").send().await?;
    let list: Vec<Torrent> = resp.error_for_status()?.json().await?;
    Ok(list)
}

pub async fn get_torrent(client: &RdClient, id: &str) -> Result<Torrent> {
    let resp = client.get(&format!("/torrents/info/{id}")).send().await?;
    let t: Torrent = resp.error_for_status()?.json().await?;
    Ok(t)
}

pub async fn select_torrent_files(
    client: &RdClient,
    id: &str,
    file_ids: Vec<u32>,
) -> Result<()> {
    let ids_str: Vec<String> = file_ids.iter().map(|i| i.to_string()).collect();
    let ids_param = if ids_str.is_empty() {
        "all".to_string()
    } else {
        ids_str.join(",")
    };
    client
        .post(&format!("/torrents/selectFiles/{id}"))
        .form(&[("files", ids_param)])
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn delete_torrent(client: &RdClient, id: &str) -> Result<()> {
    client
        .delete(&format!("/torrents/delete/{id}"))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub async fn get_rd_downloads(client: &RdClient) -> Result<Vec<RdDownload>> {
    let resp = client.get("/downloads").send().await?;
    let list: Vec<RdDownload> = resp.error_for_status()?.json().await?;
    Ok(list)
}

pub async fn delete_rd_download(client: &RdClient, id: &str) -> Result<()> {
    client
        .delete(&format!("/downloads/delete/{id}"))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
