use std::time::Duration;

use rig::{
    agent::Agent,
    completion::{Chat, CompletionModel},
    message::Message,
};

pub struct AutonomousAgent<M, Func>
where
    M: CompletionModel,
{
    /// Your agent.
    agent: Agent<M>,
    /// An async function that takes a &str and returns a bool.
    exit_condition: Func,
    /// The number of rounds that your autonomous agent may go through before it stops.
    /// Use this as a failsafe in case it's possible for your agent to never reach the exit condition
    max_turns: u32,
    /// Internal chat history.
    chat_history: Vec<Message>,
    /// The amount of delay between rounds, in seconds.
    delay_between_rounds: u64,
}

impl<M, Func, Fut> AutonomousAgent<M, Func>
where
    M: CompletionModel,
    Func: Fn(&str) -> Fut,
    Fut: Future<Output = bool> + Send,
{
    pub fn new(agent: Agent<M>, exit_condition: Func) -> Self {
        Self {
            agent,
            exit_condition,
            max_turns: 0,
            chat_history: Vec::new(),
            delay_between_rounds: 0,
        }
    }

    pub async fn run(&mut self, prompt: &str) -> Result<String, anyhow::Error> {
        let mut res = prompt.to_owned();
        let mut turns_taken = self.max_turns;
        let mut interval = tokio::time::interval(Duration::from_secs(self.delay_between_rounds));
        loop {
            if turns_taken > self.max_turns {
                tracing::info!("Turns taken exceeded max turns: {}", self.max_turns);
                break;
            }
            turns_taken += 1;
            res = self.agent.chat(&res, self.chat_history.clone()).await?;

            if (self.exit_condition)(&res).await {
                break;
            }

            interval.tick().await;
        }

        Ok(res)
    }
}
