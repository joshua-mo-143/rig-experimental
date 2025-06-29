//! This module provides an abstraction for semantic routing.
//!
//! Example usage can be found in the `routing` example on the repository: <https://github.com/joshua-mo-143/rig-extra/blob/main/examples/routing.rs>
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use rig::{
    agent::Agent,
    completion::{CompletionModel, Prompt},
    vector_store::VectorStoreIndex,
};

/// The core semantic router abstraction.
/// Contains a vector store index and a cosine similarity score threshold.
pub struct SemanticRouter<V> {
    store: V,
    threshold: f64,
}

/// An abstraction over [`SemanticRouter`] that additionally contains Rig agents.
/// Currently, each agent must be of the same completion model.
pub struct SemanticRouterWithAgents<V, M: CompletionModel> {
    store: V,
    threshold: f64,
    agents: HashMap<String, Agent<M>>,
}

impl<V> SemanticRouter<V> {
    /// Create an instance of [`SemanticRouterBuilder`].
    pub fn builder() -> SemanticRouterBuilder<V> {
        SemanticRouterBuilder::new()
    }
}

impl<V> SemanticRouter<V>
where
    V: VectorStoreIndex,
{
    pub async fn prompt(&self, query: &str) -> Option<String> {
        let res = self.store.top_n(query, 1).await.ok()?;
        let (score, _, SemanticRoute { tag }) = res.first()?;

        tracing::info!("Retrieved route: {tag}, {score}");

        if *score < self.threshold {
            return None;
        }

        Some(tag.to_owned())
    }

    pub fn agent<M: CompletionModel>(
        self,
        route: &str,
        agent: Agent<M>,
    ) -> SemanticRouterWithAgents<V, M> {
        let mut agents = HashMap::new();
        agents.insert(route.to_string(), agent);
        SemanticRouterWithAgents {
            store: self.store,
            threshold: self.threshold,
            agents,
        }
    }
}

impl<V, M> SemanticRouterWithAgents<V, M>
where
    V: VectorStoreIndex,
    M: CompletionModel,
{
    /// P
    pub async fn prompt<R>(&self, query: R) -> Result<Option<String>, Box<dyn std::error::Error>>
    where
        R: Into<RouterRequest>,
    {
        let RouterRequest { query, turns } = query.into();
        let res = self.store.top_n(&query, 1).await?;
        let (score, _, SemanticRoute { tag }) = if let Some(result) = res.first() {
            result
        } else {
            return Ok(None);
        };

        if *score < self.threshold {
            return Ok(None);
        }

        let Some(agent) = self.agents.get(tag) else {
            panic!("Couldn't find an agent that exists at tag: {tag}");
        };

        let res = if turns > 0 {
            agent
                .prompt(query)
                .multi_turn(turns as usize)
                .await
                .unwrap()
        } else {
            agent.prompt(query).await.unwrap()
        };

        Ok(Some(res))
    }

    pub fn agent(mut self, route: &str, agent: Agent<M>) -> Self {
        self.agents.insert(route.to_string(), agent);
        self
    }
}

pub struct RouterRequest {
    query: String,
    turns: u64,
}

impl RouterRequest {
    pub fn new(query: String) -> Self {
        Self::from(query)
    }

    pub fn with_turns(mut self, turns: u64) -> Self {
        self.turns = turns;
        self
    }
}

impl From<String> for RouterRequest {
    fn from(value: String) -> Self {
        Self {
            query: value,
            turns: 0,
        }
    }
}

impl From<&str> for RouterRequest {
    fn from(value: &str) -> Self {
        Self {
            query: value.to_string(),
            turns: 0,
        }
    }
}

impl From<(String, u64)> for RouterRequest {
    fn from((query, turns): (String, u64)) -> Self {
        Self { query, turns }
    }
}

impl From<(&str, u64)> for RouterRequest {
    fn from((query, turns): (&str, u64)) -> Self {
        Self {
            query: query.to_string(),
            turns,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SemanticRoute {
    tag: String,
}

pub trait Router: VectorStoreIndex {
    fn retrieve_route() -> impl std::future::Future<Output = Option<String>> + Send;
}

pub struct SemanticRouterBuilder<V> {
    store: Option<V>,
    threshold: Option<f64>,
}

impl<V> Default for SemanticRouterBuilder<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> SemanticRouterBuilder<V> {
    pub fn new() -> Self {
        Self {
            store: None,
            threshold: None,
        }
    }

    pub fn store(mut self, router: V) -> Self {
        self.store = Some(router);

        self
    }

    pub fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = Some(threshold);

        self
    }

    pub fn build(self) -> Result<SemanticRouter<V>, SemanticRouterError> {
        let Some(store) = self.store else {
            return Err(SemanticRouterError::StoreNotFound);
        };

        let threshold = self.threshold.unwrap_or(0.8);

        Ok(SemanticRouter { store, threshold })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SemanticRouterError {
    #[error("Vector store not found")]
    StoreNotFound,
}
