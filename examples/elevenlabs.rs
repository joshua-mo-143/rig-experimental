use rig::client::{ProviderClient, audio_generation::AudioGenerationClientDyn};
use rig_experimental::providers::elevenlabs::{self, ELEVEN_MULTILINGUAL_V2};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let elevenlabs_client = elevenlabs::audiogen::Client::from_env();

    let params = elevenlabs::audiogen::ElevenLabsParams::default().into_json()?;
    let res = elevenlabs_client
        .audio_generation_model(ELEVEN_MULTILINGUAL_V2)
        .audio_generation_request()
        .text("Hello world!")
        .speed(1.0)
        .voice("placeholder") // Voice IDs can be found directly from the ElevenLabs website.
        .additional_params(params)
        .send()
        .await
        .unwrap();

    std::fs::write("foo.mp3", &res.audio)?;

    Ok(())
}
