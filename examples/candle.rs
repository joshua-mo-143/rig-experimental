//! An example of using Candle as a provider.
//!
//! As you can see in the example below, it is quite easy to use.
//! Simply generate a client, set up your agent and prompt it.
use rig::client::ProviderClient;
use rig::{client::completion::CompletionClientDyn, completion::Prompt};
use rig_extra::providers::candle::{Mistral, completion::Client};

#[tokio::main]
async fn main() {
    let client: Client<Mistral> = rig_extra::providers::candle::completion::Client::from_env();

    let agent = client
        .agent("mistralai/Mistral-7B-Instruct-v0.2")
        .preamble("You are a helpful assistant")
        .build();

    let res = agent.prompt("Hello world!").await.unwrap();

    println!("Mistral: {res}");
}
