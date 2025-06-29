use rig::client::{ProviderClient, audio_generation::AudioGenerationClientDyn};
use rig_voice_agent::providers::elevenlabs::ElevenLabsParams;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let elevenlabs_client = rig_voice_agent::providers::elevenlabs::Client::from_env();

    let params = ElevenLabsParams::default().into_json()?;
    let res = elevenlabs_client
        .audio_generation_model("meme")
        .audio_generation_request()
        .text("Hello world!")
        .speed(1.0)
        .voice("meme")
        .additional_params(params)
        .send()
        .await
        .unwrap();

    std::fs::write("foo.mp3", &res.audio)?;

    Ok(())
}
