//! An example of how you can use the Realtime OpenAI API to send and receive audio bytes.
//!
//! In production, you would typically use something like CPAL and stream the audio bytes into the sender
//! (or potentially something else, depending on what you're trying to do).
//! You would then open the stream and convert the received audio deltas into bytes then use something like rodio to play the soundbytes back.
use rig_voice_agent::providers::openai_realtime::realtime::{
    RealtimeClient, RealtimeVoice, RealtimeVoiceRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let openai_client =
        rig_voice_agent::providers::openai_realtime::Client::new("1234").realtime_client("test");

    let req = RealtimeVoiceRequest::default();

    let (_sender, _stream) = openai_client.realtime_voice(req).await;

    Ok(())
}
