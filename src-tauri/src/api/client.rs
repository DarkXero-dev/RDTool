use anyhow::Result;
use reqwest::{Client, RequestBuilder};

const RD_BASE: &str = "https://api.real-debrid.com/rest/1.0";

pub struct RdClient {
    client: Client,
    token: String,
}

impl RdClient {
    pub fn new(token: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RDTool/0.1")
                .build()
                .expect("failed to build HTTP client"),
            token,
        }
    }

    pub fn get(&self, path: &str) -> RequestBuilder {
        self.client
            .get(format!("{RD_BASE}{path}"))
            .bearer_auth(&self.token)
    }

    pub fn post(&self, path: &str) -> RequestBuilder {
        self.client
            .post(format!("{RD_BASE}{path}"))
            .bearer_auth(&self.token)
    }

    pub fn put(&self, path: &str) -> RequestBuilder {
        self.client
            .put(format!("{RD_BASE}{path}"))
            .bearer_auth(&self.token)
    }

    pub fn delete(&self, path: &str) -> RequestBuilder {
        self.client
            .delete(format!("{RD_BASE}{path}"))
            .bearer_auth(&self.token)
    }
}

pub fn build_client(token: String) -> Result<RdClient> {
    Ok(RdClient::new(token))
}
