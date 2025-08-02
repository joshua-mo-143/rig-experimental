use futures::{SinkExt, StreamExt, stream::BoxStream};
use reqwest_websocket::Message;
use rig::providers::openai::ToolDefinition;
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

    pub fn with_session(session: Session) -> Self {
        Self {
            session: Some(session),
        }
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

        if let Some(session) = req.session {
            tx.send(InputEvent::update_session(session))
                .await
                .expect("If this closes, there was a malformed JSON object sent to OpenAI");
        }

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
    /// Update a session. Note that only fields with Some will be updated - anything else will be left blank.
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
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
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
    /// Turn detection. Instead of manually committing, this allows OpenAI to just figure it out for you.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<TurnDetection>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn voice(mut self, voice: &str) -> Self {
        self.voice = Some(voice.to_string());
        self
    }

    pub fn instructions(mut self, instructions: &str) -> Self {
        self.instructions = Some(instructions.to_string());
        self
    }

    pub fn turn_detection(mut self, cfg: TurnDetection) -> Self {
        self.turn_detection = Some(cfg);
        self
    }

    pub fn input_audio_format(mut self, format: AudioFormat) -> Self {
        self.input_audio_format = Some(format);
        self
    }

    pub fn output_audio_format(mut self, format: AudioFormat) -> Self {
        self.output_audio_format = Some(format);
        self
    }

    pub fn modalities(mut self, arr: Vec<Modality>) -> Self {
        self.modalities = Some(arr);
        self
    }

    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }
}

/// Turn detection config.
/// If you don't want to configure manual audio commits, this is a useful convenience method for having OpenAI do it entirely for you.
/// Instead of manually committing, you simply append audio inputs and OpenAI figures it out.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TurnDetection {
    /// The turn detection kind. T
    #[serde(rename = "type")]
    kind: Option<TurnDetectionKind>,
    threshold: Option<f64>,
    prefix_padding_ms: Option<u64>,
    silence_duration_ms: Option<u64>,
    create_response: Option<bool>,
}

impl TurnDetection {
    /// Creates a new TurnDetection config with defaults from the OpenAI examples.
    pub fn with_openai_defaults() -> Self {
        Self {
            kind: Some(TurnDetectionKind::ServerVad),
            threshold: Some(0.5),
            prefix_padding_ms: Some(300),
            silence_duration_ms: Some(500),
            create_response: Some(true),
        }
    }

    /// Creates a new empty TurnDetection config.
    pub fn empty() -> Self {
        Self {
            kind: Some(TurnDetectionKind::ServerVad),
            threshold: None,
            prefix_padding_ms: None,
            silence_duration_ms: None,
            create_response: None,
        }
    }

    /// Add a dB threshold to your turn detection (OpenAI will use this to figure out what sound threshold to start detecting when the LLM's turn is)
    pub fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Add a grace duration during which there will be padding (ie no audio).
    pub fn prefix_padding_ms(mut self, prefix_padding_ms: u64) -> Self {
        self.prefix_padding_ms = Some(prefix_padding_ms);
        self
    }

    /// Add a duration for which silence is required (ie the threshold needs to be met or exceeded) for OpenAI turn detection.
    pub fn silence_duration_ms(mut self, silence_duration_ms: u64) -> Self {
        self.silence_duration_ms = Some(silence_duration_ms);
        self
    }

    /// Whether or not a response should be created. This should be true by default if you are initialising a session.
    pub fn create_response(mut self, create_response: bool) -> Self {
        self.create_response = Some(create_response);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TurnDetectionKind {
    #[serde(rename = "server_vad")]
    #[default]
    ServerVad,
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
