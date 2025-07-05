//! An example of how you can use the Realtime OpenAI API to send and receive audio bytes, but using CPAL.
//! This example uses OpenAI's Speech Activity Detection and as such does not need to manually commit audio buffers (by sending the relevant input event).
use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink};
use rubato::{FftFixedInOut, Resampler};

use std::sync::Arc;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use futures::StreamExt;
use rig_experimental::providers::openai_realtime::realtime::{
    AudioFormat, GPT_4O_REALTIME_PREVIEW_20250603, InputEvent, Modality, ReceivedEventKind,
    ReceivedItemEventKind, Session, SessionEvent, TurnDetection,
};
use rig_experimental::providers::openai_realtime::{
    Client,
    realtime::{RealtimeClient, RealtimeVoice, RealtimeVoiceRequest},
};

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY env var should exist");
    let openai_client = Client::new(&api_key).realtime_client(GPT_4O_REALTIME_PREVIEW_20250603);

    let session = Session::new()
        .voice("sage")
        .input_audio_format(AudioFormat::Pcm16)
        .output_audio_format(AudioFormat::Pcm16)
        .modalities(vec![Modality::Text, Modality::Audio])
        .turn_detection(TurnDetection::with_openai_defaults().threshold(0.3));

    let req = RealtimeVoiceRequest::with_session(session);

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let input_stream = record_audio(tx).await.expect("Audio stream setup failed");
    input_stream.play()?;

    let (sender, mut stream) = openai_client.realtime_voice(req).await?;

    let sender_clone = sender.clone();
    tokio::spawn(async move {
        while let Some(str) = rx.recv().await {
            sender_clone
                .send(InputEvent::append_audio(&str))
                .await
                .unwrap();
        }
    });

    println!("Waiting for events...");

    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        sender.send(InputEvent::commit_audio()).await.unwrap();
    });

    while let Some(evt) = stream.next().await {
        println!("Recieved data: {evt:?}");
        match evt.data {
            ReceivedEventKind::Item {
                data: ReceivedItemEventKind::AudioDelta { delta },
                ..
            } => {
                let bytes = BASE64_STANDARD.decode(delta).unwrap();
                play_audio(bytes).unwrap();
            }
            ReceivedEventKind::Session(SessionEvent::SessionUpdated { session }) => {
                // println!("Updated session: {session:?}");
            }
            ReceivedEventKind::Session(SessionEvent::SessionCreated { session }) => {
                // println!("Created session: {session:?}");
            }
            ReceivedEventKind::Item {
                data: ReceivedItemEventKind::AudioDone,
                ..
            } => {}
        }
    }

    Ok(())
}

pub async fn record_audio(raw_tx: tokio::sync::mpsc::Sender<String>) -> anyhow::Result<Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("no input device available");
    let config = device.default_input_config()?;

    let input_sample_rate = config.sample_rate().0;
    let target_sample_rate = 24_000;
    let channels = config.channels() as usize;

    // Real-time safe send-only channel to bridge audio thread → async task
    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

    // Spawn async side processor
    tokio::spawn(async move {
        let mut resampler =
            FftFixedInOut::<f32>::new(input_sample_rate as usize, target_sample_rate, 1024, 1)
                .expect("Failed to init resampler");

        while let Ok(mono_chunk) = audio_rx.recv() {
            println!("Received chunks");
            let input = vec![mono_chunk];
            let Ok(resampled) = resampler.process(&input, None) else {
                continue;
            };
            let output = &resampled[0];

            let pcm_i16: Vec<i16> = output
                .iter()
                .map(|s| (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16)
                .collect();

            let encoded = BASE64_STANDARD.encode(bytemuck::cast_slice(&pcm_i16));
            let _ = raw_tx.send(encoded).await;
        }
    });

    let err_fn = |err| eprintln!("Stream error: {err}");

    let audio_tx = Arc::new(audio_tx);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let audio_tx = Arc::clone(&audio_tx);

            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    println!("Got chunks!");
                    let mut mono: Vec<f32> = Vec::with_capacity(data.len() / channels);
                    for frame in data.chunks(channels) {
                        let avg = frame.iter().sum::<f32>() / channels as f32;
                        mono.push(avg);
                    }
                    println!("Sending chunks...");

                    let _ = audio_tx.send(mono); // don't block
                },
                err_fn,
                None,
            )?
        }
        _ => unimplemented!("Only f32 input supported"),
    };

    Ok(stream)
}

fn play_audio(bytes: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let samples: &[i16] = bytemuck::cast_slice(&bytes);
    let source = SamplesBuffer::new(1, 24_000, samples.to_vec());
    sink.append(source);

    sink.sleep_until_end(); // Wait until playback finishes
    Ok(())
}
