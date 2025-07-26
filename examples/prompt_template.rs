//! An example of using prompt templating.
//!
//! As you can see in the example below, it is quite easy to use.
//! Simply generate a client, set up your agent and prompt it.
use rig::client::ProviderClient;
use rig::{client::completion::CompletionClientDyn, providers::openai};

// PromptTemplating trait
use rig_experimental::prompt_templating::PromptTemplating;

#[tokio::main]
async fn main() {
    let client = openai::Client::from_env();

    let agent = client
        .agent("gpt-4o")
        .preamble(
            "You are a helpful assistant. Ensure you call the user by their name when responding.",
        )
        .build();

    let res = agent
        .with_prompt_template(TEMPLATE)
        .with_variable("user", "Rig")
        .prompt()
        .await
        .unwrap();

    println!("GPT-4o: {res}");
}

const TEMPLATE: &str = "Hello, ChatGPT! My name is {{ user }}!";
