use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use rig::{
    agent::Agent,
    completion::{CompletionModel, Prompt},
    vector_store::VectorStoreIndex,
};

pub struct SemanticRouter<V> {
    store: V,
    threshold: f64,
}

pub struct SemanticRouterWithAgents<V, M: CompletionModel> {
    store: V,
    threshold: f64,
    agents: HashMap<String, Agent<M>>,
}

impl<V> SemanticRouter<V> {
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
    pub async fn prompt(&self, query: &str) -> Option<String> {
        let res = self.store.top_n(query, 1).await.ok()?;
        let (score, _, SemanticRoute { tag }) = res.first()?;

        if *score < self.threshold {
            return None;
        }

        let Some(agent) = self.agents.get(tag) else {
            panic!("Couldn't find an agent that exists at tag: {tag}");
        };

        let res = agent.prompt(query).await.unwrap();
        Some(res)
    }

    pub fn agent(mut self, route: &str, agent: Agent<M>) -> Self {
        self.agents.insert(route.to_string(), agent);
        self
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
