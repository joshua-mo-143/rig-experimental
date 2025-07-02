use futures::{SinkExt, StreamExt, stream::BoxStream};
use reqwest_websocket::Message;
use rig::providers::openai::{InputAudio, ToolDefinition};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Sender};

pub trait RealtimeVoice: Clone {
    fn realtime_voice(
        &self,
        req: RealtimeVoiceRequest,
    ) -> impl Future<
        Output = Result<
            (Sender<InputEvent>, BoxStream<'_, ReceivedEvent>),
            Box<dyn std::error::Error>,
        >,
    > + Send;
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RealtimeVoiceRequest {
    session: Option<Session>,
}

impl RealtimeVoiceRequest {
    pub fn new() -> Self {
        Self { session: None }
    }

    pub fn session_data(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }
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
        req: RealtimeVoiceRequest,
    ) -> Result<(Sender<InputEvent>, BoxStream<'_, ReceivedEvent>), Box<dyn std::error::Error>>
    {
        let path = format!("/realtime?model={model_id}", model_id = self.model);
        let websocket = self
            .client
            .initiate_websocket(&path)
            .await
            .inspect_err(|x| println!("Error: {x}"))
            .unwrap();

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
                        tracing::debug!("Received text: {txt}");
                        serde_json::from_str::<ReceivedEvent>(&txt).ok()
                    }
                    Err(err) => {
                        tracing::debug!("Received error: {err}");
                        None
                    }
                    Ok(thing) => {
                        tracing::debug!(
                            "Got thing that was neither a text message nor an error: {thing:?}"
                        );
                        None
                    }
                }
            })
            .boxed();

        Ok((tx, mapped_stream))
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
        Self::new(InputEventKind::AppendAudioInput {
            audio: input.to_string(),
        })
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.event_id = Some(id.to_string());
        self
    }

    pub fn update_session(session: Session) -> Self {
        Self::new(InputEventKind::UpdateSession { session })
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
    AppendAudioInput { audio: String },
    #[serde(rename = "session.update")]
    UpdateSession { session: Session },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReceivedEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    #[serde(flatten)]
    pub data: ReceivedEventKind,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ReceivedEventKind {
    Session(SessionEvent),
    Item {
        item_id: String,
        response_id: String,
        output_index: u64,
        content_index: u64,
        #[serde(flatten)]
        data: ReceivedItemEventKind,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SessionEvent {
    #[serde(rename = "session.created")]
    SessionCreated { session: Session },
    #[serde(rename = "session.updated")]
    SessionUpdated { session: Session },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ReceivedItemEventKind {
    /// An audio delta containing a base64-encoded string of the audio bytes.
    #[serde(rename = "response.audio.delta")]
    AudioDelta { delta: String },
    /// Clears all audio bytes from the input buffer.
    #[serde(rename = "response.audio.done")]
    AudioDone,
}

/// The gpt-4o-realtime-preview-2025-06-03 model. For use with the OpenAI realtime API.
pub const GPT_4O_REALTIME_PREVIEW_20250603: &str = "gpt-4o-realtime-preview-2025-06-03";

/// OpenAI's realtime API session data. You can use this to update the realtime session at any time.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Session {
    /// The modality you want to use. Can either be text or audio (and you can have both in the same vec!)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<Modality>>,
    /// The preamble ("system prompt") that you want to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// The OpenAI voice you want to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// The input audio format to be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_format: Option<AudioFormat>,
    /// The output audio format to be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_audio_format: Option<AudioFormat>,
    /// The model you want to use for input audio transcription.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_transcription: Option<InputAudioTranscription>,
    /// The tools you want to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// The temperature you want to use. Set higher for more creative responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Playback speed.
    pub speed: f64,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            modalities: None,
            instructions: None,
            voice: None,
            input_audio_format: None,
            output_audio_format: None,
            input_audio_transcription: None,
            tools: None,
            temperature: None,
            speed: 1.0,
        }
    }
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn voice(mut self, voice: &str) -> Self {
        self.voice = Some(voice.to_string());
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputAudioTranscription {
    model: String,
}

impl Default for InputAudioTranscription {
    fn default() -> Self {
        Self {
            model: "whisper-1".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Pcm16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modality {
    Text,
    Audio,
}
