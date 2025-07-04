//! An example of how you can use the Realtime OpenAI API to send and receive audio bytes, but using CPAL.
//! This example uses OpenAI's Speech Activity Detection and as such does not need to manually commit audio buffers (by sending the relevant input event).
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::{Decoder, OutputStream, Sink};
use std::sync::Mutex;
use tokio::sync::mpsc::Sender;

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures::StreamExt;
use rig_experimental::providers::openai_realtime::realtime::{
    AudioFormat, GPT_4O_REALTIME_PREVIEW_20250603, InputEvent, Modality, ReceivedEventKind,
    ReceivedItemEventKind, Session, SessionEvent,
};
use rig_experimental::providers::openai_realtime::{
    Client,
    realtime::{RealtimeClient, RealtimeVoice, RealtimeVoiceRequest},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY env var should exist");
    let openai_client = Client::new(&api_key).realtime_client(GPT_4O_REALTIME_PREVIEW_20250603);

    let session = Session::new()
        .voice("sage")
        .input_audio_format(AudioFormat::Pcm16)
        .output_audio_format(AudioFormat::Pcm16)
        .modalities(vec![Modality::Text, Modality::Audio]);

    let req = RealtimeVoiceRequest::with_session(session);

    let (sender, mut stream) = openai_client.realtime_voice(req).await?;
    let (tx, rx) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        record_audio(tx).expect("Audio stream failed");
    });

    tokio::spawn(async move {
        while let Ok(str) = rx.recv() {
            sender.send(InputEvent::append_audio(&str)).await.unwrap();
        }
    });

    while let Some(evt) = stream.next().await {
        match evt.data {
            ReceivedEventKind::Item {
                data: ReceivedItemEventKind::AudioDelta { delta },
                ..
            } => {
                let bytes = BASE64_STANDARD.decode(delta).unwrap();
                play_audio(bytes).unwrap();
            }
            ReceivedEventKind::Session(SessionEvent::SessionUpdated { session }) => {
                println!("Updated session: {session:?}");
            }
            ReceivedEventKind::Session(SessionEvent::SessionCreated { session }) => {
                println!("Created session: {session:?}");
            }
            ReceivedEventKind::Item {
                data: ReceivedItemEventKind::AudioDone,
                ..
            } => break,
        }
    }

    Ok(())
}

fn record_audio(tx: std::sync::mpsc::Sender<String>) -> Result<(), anyhow::Error> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("No input device");
    let config = device.default_input_config()?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let tx = Arc::new(Mutex::new(tx));
    let mut buffer: Vec<i16> = Vec::with_capacity(sample_rate as usize); // 1s buffer

    let err_fn = |err| eprintln!("Stream error: {err}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let tx = tx.lock().unwrap();

                for chunk in data.chunks(channels) {
                    // Convert to mono by averaging channels
                    let mono_sample = chunk.iter().copied().sum::<f32>() / channels as f32;
                    let i16_sample = (mono_sample * i16::MAX as f32) as i16;
                    buffer.push(i16_sample);
                }

                if buffer.len() as u32 >= sample_rate / 5 {
                    // ~200ms of audio
                    let raw_bytes = bytemuck::cast_slice(&buffer).to_vec();
                    buffer.clear();
                    let input = base64::prelude::BASE64_STANDARD.encode(raw_bytes);
                    let _ = tx.send(input);
                }
            },
            err_fn,
            None,
        )?,
        _ => unimplemented!("Only f32 input is currently supported"),
    };

    stream.play()?;
    std::thread::sleep(std::time::Duration::from_secs(9999));
    Ok(())
}

fn play_audio(bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let cursor = Cursor::new(bytes);
    let source = Decoder::new(cursor)?;
    sink.append(source);

    sink.sleep_until_end(); // Wait until playback finishes
    Ok(())
}
