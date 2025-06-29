use futures::{SinkExt, StreamExt, stream::BoxStream};
use reqwest_websocket::Message;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};

pub trait RealtimeVoice: Clone {
    fn realtime_voice(
        &self,
        req: RealtimeVoiceRequest,
    ) -> impl Future<Output = (Sender<InputEvent>, BoxStream<'_, ReceivedEvent>)> + Send;
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RealtimeVoiceRequest {
    #[serde(flatten)]
    additional_params: serde_json::Value,
}

pub trait RealtimeClient {
    type Output: RealtimeVoice;

    fn realtime_client(&self, model_name: &str) -> Self::Output;
}

#[derive(Clone, Debug)]
pub struct RealtimeModel {
    client: super::client::Client,
    model: String,
}

impl RealtimeModel {
    pub fn new(client: super::client::Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
        }
    }
}

impl RealtimeVoice for RealtimeModel {
    async fn realtime_voice(
        &self,
        _req: RealtimeVoiceRequest,
    ) -> (Sender<InputEvent>, BoxStream<'_, ReceivedEvent>) {
        let path = format!("/realtime?model={model_id}", model_id = self.model);
        let websocket = self.client.initiate_websocket(&path).await.unwrap();

        let (mut ws_tx, ws_rx) = websocket.split();

        let (tx, mut rx) = mpsc::channel::<InputEvent>(9999);

        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                let json = serde_json::to_string(&message).unwrap();
                ws_tx.send(Message::Text(json)).await.unwrap();
            }
        });

        // Convert `ws_rx` (Stream of WebSocket messages) into a stream of `ReceivedEvent`
        let mapped_stream = ws_rx
            .filter_map(|msg_result| async {
                match msg_result {
                    Ok(reqwest_websocket::Message::Text(txt)) => {
                        serde_json::from_str::<ReceivedEvent>(&txt).ok()
                    }
                    _ => None, // Skip non-text messages or errors
                }
            })
            .boxed();

        (tx, mapped_stream)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    #[serde(flatten)]
    data: InputEventKind,
}

impl InputEvent {
    pub fn new(data: InputEventKind) -> Self {
        Self {
            event_id: None,
            data,
        }
    }

    pub fn commit_audio() -> Self {
        Self::new(InputEventKind::CommitAudioInputBuffer)
    }

    pub fn clear_audio() -> Self {
        Self::new(InputEventKind::ClearAudioInputBuffer)
    }

    pub fn append_audio(input: &str) -> Self {
        Self::new(InputEventKind::AppendAudioInput(input.to_string()))
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.event_id = Some(id.to_string());
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum InputEventKind {
    /// Commit the user input audio buffer, which will create a new user message item in the conversation. Produces an error if there is nothing in the audio stream.
    #[serde(rename = "input_audio_buffer.commit")]
    CommitAudioInputBuffer,
    /// Clears all audio bytes from the input buffer.
    #[serde(rename = "input_audio_buffer.clear")]
    ClearAudioInputBuffer,
    /// Append audio input. Takes a Base64 encoded set of bytes.
    #[serde(rename = "input_audio_buffer.append")]
    AppendAudioInput(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReceivedEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    item_id: String,
    output_index: String,
    content_index: String,
    #[serde(flatten)]
    data: ReceivedEventKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ReceivedEventKind {
    /// An audio delta containing a base64-encoded string of the audio bytes.
    #[serde(rename = "response_audio.delta")]
    AudioDelta(String),
    /// Clears all audio bytes from the input buffer.
    #[serde(rename = "response_audio.done")]
    AudioDone,
}
