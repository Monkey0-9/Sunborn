//! Agent-to-Agent (A2A) Gateway & Agent Card Discovery (AgentGateway & AISIX inspired).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub id: String,
    pub name: String,
    pub description: String,
    pub endpoint_url: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskRequest {
    pub agent_id: String,
    pub task_id: Option<String>,
    pub input: serde_json::Value,
}

#[derive(Clone)]
pub struct A2aGateway {
    agents: Arc<RwLock<HashMap<String, AgentCard>>>,
    client: reqwest::Client,
}

impl Default for A2aGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl A2aGateway {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            client,
        }
    }

    pub fn register_agent(&self, card: AgentCard) -> Result<AgentCard, String> {
        if card.id.trim().is_empty() {
            return Err("Agent ID cannot be empty".into());
        }
        crate::mcp_gateway::McpGateway::validate_upstream_url(&card.endpoint_url)?;
        let mut agents = self.agents.write().unwrap_or_else(|e| e.into_inner());
        agents.insert(card.id.clone(), card.clone());
        Ok(card)
    }

    pub fn list_agents(&self) -> Vec<AgentCard> {
        self.agents
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    pub fn well_known_card(&self) -> serde_json::Value {
        let agents = self.list_agents();
        let all_skills: Vec<String> = agents
            .iter()
            .filter(|a| a.enabled)
            .flat_map(|a| a.skills.clone())
            .collect();
        serde_json::json!({
            "name": "Sunborn A2A Gateway",
            "version": env!("CARGO_PKG_VERSION"),
            "capabilities": { "streaming": false, "pushNotifications": false },
            "skills": all_skills,
            "registeredAgents": agents.len(),
        })
    }

    pub async fn send_task(&self, req: A2aTaskRequest) -> Result<serde_json::Value, String> {
        let card = {
            let guard = self.agents.read().unwrap_or_else(|e| e.into_inner());
            guard.get(&req.agent_id).cloned()
        }
        .ok_or_else(|| format!("A2A agent '{}' not found", req.agent_id))?;

        if !card.enabled {
            return Err(format!("A2A agent '{}' is disabled", req.agent_id));
        }

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req.task_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "method": "tasks/send",
            "params": { "message": req.input }
        });

        let resp = self
            .client
            .post(&card.endpoint_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("A2A upstream request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("A2A upstream HTTP {}", resp.status()));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("A2A upstream parse error: {}", e))
    }
}
