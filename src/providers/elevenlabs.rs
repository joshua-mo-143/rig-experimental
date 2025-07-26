//! The module for Eleven Labs.
//!

use std::fmt::{self, Debug};

use rig::{
    audio_generation::{self, AudioGenerationError},
    client::{AudioGenerationClient, ProviderClient},
    impl_conversion_traits,
};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Client {
    base_url: String,
    api_key: String,
    http_client: reqwest::Client,
}

impl Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("http_client", &self.http_client)
            .field("api_key", b"<REDACTED>")
            .finish()
    }
}

const ELEVENLABS_API_BASE_URL: &str = "https://api.elevenlabs.io/v1";

impl Client {
    pub fn new(api_key: &str) -> Self {
        Self::from_url(api_key, ELEVENLABS_API_BASE_URL)
    }

    fn from_url(api_key: &str, url: &str) -> Self {
        Self {
            base_url: url.to_string(),
            api_key: api_key.to_string(),
            http_client: reqwest::Client::builder()
                .build()
                .expect("The ElevenLabs client should always build correctly"),
        }
    }

    pub fn with_custom_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = client;
        self
    }

    async fn post<T>(&self, path: &str, body: &T) -> Result<reqwest::Response, reqwest::Error>
    where
        T: serde::Serialize,
    {
        let mut url = self.base_url.clone();
        url.push_str(path);

        self.http_client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .json(body)
            .send()
            .await
    }
}

impl ProviderClient for Client {
    fn from_env() -> Self
    where
        Self: Sized,
    {
        let api_key = std::env::var("ELEVENLABS_API_KEY")
            .expect("expected ELEVENLABS_API_KEY to exist as an environment variable");

        Self::new(&api_key)
    }
}

impl AudioGenerationClient for Client {
    type AudioGenerationModel = AudioGenerationModel;
    /// Create an audio generation model with the given name.
    ///
    /// # Example
    /// ```
    /// use rig_experimental::providers::elevenlabs::{Client, self};
    /// use rig::client::AudioGenerationClient;
    ///
    /// // Initialize the ElevenLabs client
    /// let elevenlabs = Client::new("your-elevenlabs-api-key");
    ///
    /// let model = openai.audio_generation_model(elevenlabs::ELEVEN_MULTILINGUAL_V2);
    /// ```
    fn audio_generation_model(&self, model: &str) -> Self::AudioGenerationModel {
        AudioGenerationModel::new(self.clone(), model)
    }
}

impl_conversion_traits!(AsCompletion, AsEmbeddings, AsTranscription, AsImageGeneration for Client);

#[derive(Clone, Debug)]
pub struct AudioGenerationModel {
    client: Client,
    model: String,
}

impl AudioGenerationModel {
    fn new(client: Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioGenerationRequest {
    /// The text that will be used to generate speech
    pub text: String,
    /// The audio generation model that will be used to generate speech
    pub model_id: String,
    /// The ID of the voice that will be used to generate speech
    pub voice_id: String,
    #[serde(flatten)]
    pub params: ElevenLabsParams,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ElevenLabsParams {
    /// The audio output format
    pub output_format: AudioOutputFormat,
    /// The language code (ISO 936-1) that the model will use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_code: Option<String>,
    /// Voice settings to be used by the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_settings: Option<VoiceSettings>,
    /// Use a pre-generated seed to help deterministically create audio samples. However, determinism is not guaranteed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// The text that came before the text of the current request. Can be used to improve speech continuity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_text: Option<String>,
    /// The text that came after the text of the current request. Can be used to improve speech continuity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_text: Option<String>,
    /// A list of requests that were generated before this generation. Can be used to improve speech continuity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_request_ids: Option<String>,
    /// A list of samples that come after this generation. Can be used to improve speech continuity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_request_ids: Option<Vec<String>>,
    /// Whether or not to apply text normalization. If an option is not supplied (or you've set it to Auto), the Elevenlabs API will decide whether or not to apply it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_text_normalization: Option<ApplyTextNormalization>,
    /// Controls language text normalization - helps with proper pronounciation of text in some supported languages. This parameter can heavily increase the latency of the request and is so far only supported in Japanese.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_language_text_normalization: Option<bool>,
}

impl ElevenLabsParams {
    pub fn into_json(self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl TryFrom<(String, audio_generation::AudioGenerationRequest)> for AudioGenerationRequest {
    type Error = AudioGenerationError;
    fn try_from(
        (model_name, req): (String, audio_generation::AudioGenerationRequest),
    ) -> Result<Self, Self::Error> {
        let audio_generation::AudioGenerationRequest {
            text,
            voice,
            speed,
            additional_params,
        } = req;

        let Some(params) = additional_params else {
            return Err(AudioGenerationError::ProviderError("You need to use additional parameters to be able to insert required variables for this provider!".into()));
        };

        let mut params: ElevenLabsParams = serde_json::from_value(params)?;
        let voice_settings = {
            let mut settings = params.voice_settings.unwrap_or_default();

            settings.speed = Some(speed as f64);
            settings
        };
        params.voice_settings = Some(voice_settings);

        // let params = params.voice_settings.map(|x| x.speed = Some(speed as f64));

        Ok(Self {
            text,
            voice_id: voice,
            model_id: model_name,
            params,
        })
    }
}

impl TryFrom<(&str, audio_generation::AudioGenerationRequest)> for AudioGenerationRequest {
    type Error = AudioGenerationError;
    fn try_from(
        (model_name, req): (&str, audio_generation::AudioGenerationRequest),
    ) -> Result<Self, Self::Error> {
        let model_name = model_name.to_string();
        Self::try_from((model_name, req))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum ApplyTextNormalization {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct VoiceSettings {
    pub stability: Option<f64>,
    pub use_speaker_boost: Option<bool>,
    pub similarity_boost: Option<f64>,
    pub style: Option<f64>,
    pub speed: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub enum AudioOutputFormat {
    /// MP3 with 22.05kHz sample at 32kbs
    #[serde(rename = "mp3_22050_32")]
    #[default]
    Mp3_22050_32,

    /// MP3 with 44.1kHz sample at 32kbs
    #[serde(rename = "mp3_44100_32")]
    Mp3_44100_32,

    /// MP3 with 44.1kHz sample at 64kbs
    #[serde(rename = "mp3_44100_64")]
    Mp3_44100_64,

    /// MP3 with 44.1kHz sample at 96kbs
    #[serde(rename = "mp3_44100_96")]
    Mp3_44100_96,

    /// MP3 with 44.1kHz sample at 128kbs
    #[serde(rename = "mp3_44100_128")]
    Mp3_44100_128,

    /// MP3 with 44.1kHz sample at 192kbs
    #[serde(rename = "mp3_44100_192")]
    Mp3_44100_192,

    /// PCM with 8kHz sample
    #[serde(rename = "pcm_8000")]
    Pcm8000,

    /// PCM with 16kHz sample
    #[serde(rename = "pcm_16000")]
    Pcm16000,

    /// PCM with 22.05kHz sample
    #[serde(rename = "pcm_22050")]
    Pcm22050,

    /// PCM with 44.1kHz sample
    #[serde(rename = "pcm_44100")]
    Pcm44100,

    /// PCM with 48kHz sample
    #[serde(rename = "pcm_48000")]
    Pcm48000,

    /// ULaw with 8kHz sample
    #[serde(rename = "ulaw_8000")]
    Ulaw8000,

    /// ALaw with 8kHz sample
    #[serde(rename = "alaw_8000")]
    Alaw8000,

    /// Opus with 48kHz sample at 32kbs
    #[serde(rename = "opus_48000_32")]
    Opus4800032,

    /// Opus with 48kHz sample at 64kbs
    #[serde(rename = "opus_48000_64")]
    Opus4800064,

    /// Opus with 48kHz sample at 96kbs
    #[serde(rename = "opus_48000_96")]
    Opus4800096,

    /// Opus with 48kHz sample at 128kbs
    #[serde(rename = "opus_48000_128")]
    Opus48000128,

    /// Opus with 48kHz sample at 192kbs
    #[serde(rename = "opus_48000_192")]
    Opus48000192,
}

impl audio_generation::AudioGenerationModel for AudioGenerationModel {
    type Response = Bytes;

    async fn audio_generation(
        &self,
        request: audio_generation::AudioGenerationRequest,
    ) -> Result<
        audio_generation::AudioGenerationResponse<Self::Response>,
        audio_generation::AudioGenerationError,
    > {
        let req: AudioGenerationRequest =
            AudioGenerationRequest::try_from((self.model.as_ref(), request))?;
        let url = format!(
            "/text-to-speech/{voice_id}?output_format={output}",
            voice_id = req.voice_id,
            output =
                serde_json::to_string(&req.params.output_format).expect("This should never fail")
        );

        let response = self.client.post(&url, &req).await.unwrap().bytes().await?;

        Ok(audio_generation::AudioGenerationResponse {
            audio: response.to_vec(),
            response,
        })
    }
}

/// The ElevenLabs eleven_multilingual_v2 model.
pub const ELEVEN_MULTILINGUAL_V2: &str = "eleven_multilingual_v2";

/// The ElevenLabs eleven_v3 model.
pub const ELEVEN_V3: &str = "eleven_v3";

/// The ElevenLabs eleven_flash_v2 model.
pub const ELEVEN_FLASH_V2: &str = "eleven_flash_v2";

/// The ElevenLabs eleven_turbo_v2_5 model.
pub const ELEVEN_TURBO_V2_5: &str = "eleven_turbo_v2_5";

/// The ElevenLabs scribe_v1 model for usage with transcription.
pub const SCRIBE_V1: &str = "scribe_v1";
