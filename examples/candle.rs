use rig::client::ProviderClient;
use rig::{client::completion::CompletionClientDyn, completion::Prompt};
use rig_extra::providers::candle::{Mistral, completion::Client};

#[tokio::main]
async fn main() {
    let client: Client<Mistral> = rig_extra::providers::candle::completion::Client::from_env();

    let agent = client
        .agent("mistralai/Mistral-7B-v0.1")
        .preamble("You are a helpful assistant")
        .build();

    let res = agent.prompt("Hello world!").await.unwrap();

    println!("Mistral: {res}");
}
