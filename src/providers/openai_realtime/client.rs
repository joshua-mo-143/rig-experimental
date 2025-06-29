use reqwest_websocket::RequestBuilderExt;

use super::realtime::{RealtimeClient, RealtimeModel};

const OPENAI_WSS_BASE_URL: &str = "wss://api.openai.com/v1";

#[derive(Debug, Clone)]
pub struct Client {
    api_key: String,
    base_url: String,
    http_client: reqwest::Client,
}

impl Client {
    pub fn new(api_key: &str) -> Self {
        Self::from_url(api_key, OPENAI_WSS_BASE_URL)
    }

    pub fn from_url(api_key: &str, base_url: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            http_client: reqwest::Client::builder()
                .build()
                .expect("This should build!"),
        }
    }

    pub async fn initiate_websocket(
        &self,
        path: &str,
    ) -> Result<reqwest_websocket::WebSocket, reqwest_websocket::Error> {
        let url = format!("{base_url}{path}", base_url = self.base_url);
        let response = self
            .http_client
            .post(url)
            .bearer_auth(&self.api_key)
            .header("OpenAI-Beta", "realtime=v1")
            .upgrade()
            .send()
            .await?;

        response.into_websocket().await
    }
}

impl RealtimeClient for super::client::Client {
    type Output = RealtimeModel;

    fn realtime_client(&self, model_name: &str) -> Self::Output {
        RealtimeModel::new(self.clone(), model_name)
    }
}
