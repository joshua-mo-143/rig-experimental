//! The candle provider module.
//! Currently supported models:
//! - Mistral
//!
//! Currently, only text messages are supported at the moment and this is reflected in the implementation of the module.
//! You will also need to ensure your model supports EOS tokens for optimal results, as otherwise this may lead to the model effectively continuing to write until its token limit.
//!
//! An example of how to use this module with Mistral (requires a HuggingFace API key to access the model listed in the agent):
//!
//! ```rust
//! use rig::client::ProviderClient;
//! use rig::{client::completion::CompletionClientDyn, completion::Prompt};
//! use rig_extra::providers::candle::{Mistral, completion::Client};
//!
//! #[tokio::main]
//! async fn main() {
//!     let client: Client<Mistral> = rig_extra::providers::candle::completion::Client::from_env();
//!
//!     let agent = client
//!         .agent("mistralai/Mistral-7B-Instruct-v0.2")
//!         .preamble("You are a helpful assistant")
//!         .build();
//!
//!     let res = agent.prompt("Hello world!").await.unwrap();
//!
//!     println!("Mistral: {res}");
//! }
//! ```
pub mod completion;

pub use candle_transformers::models::mistral::Model as Mistral;
