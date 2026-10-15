use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// Record of an autonomic self-healing incident and automated recovery action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingIncident {
    pub incident_id: String,
    pub component: String,
    pub trigger_reason: String,
    pub recovery_action: String,
    pub resolved_at: DateTime<Utc>,
}

/// Level 7 Autonomic Self-Healing & Continuous Closed-Loop Controller
pub struct AutonomicSelfHealingController {
    incidents: RwLock<Vec<HealingIncident>>,
}

impl Default for AutonomicSelfHealingController {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomicSelfHealingController {
    pub fn new() -> Self {
        Self {
            incidents: RwLock::new(Vec::new()),
        }
    }

    /// Record an automated self-healing action
    pub fn record_recovery(&self, component: &str, trigger: &str, action: &str) {
        let incident = HealingIncident {
            incident_id: uuid::Uuid::new_v4().to_string(),
            component: component.to_string(),
            trigger_reason: trigger.to_string(),
            recovery_action: action.to_string(),
            resolved_at: Utc::now(),
        };

        let mut list = self.incidents.write().unwrap();
        list.push(incident);
    }

    /// List historical self-healing recovery actions
    pub fn list_incidents(&self) -> Vec<HealingIncident> {
        let list = self.incidents.read().unwrap();
        list.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomic_controller_records_and_retrieves_incidents() {
        let controller = AutonomicSelfHealingController::new();
        assert!(controller.list_incidents().is_empty());

        controller.record_recovery(
            "exact_cache_circuit_breaker",
            "Downstream Dragonfly timeout exceeded 3 consecutive probes",
            "Autonomously reset circuit to Closed after successful canary health probe",
        );

        let incidents = controller.list_incidents();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].component, "exact_cache_circuit_breaker");
        assert_eq!(
            incidents[0].trigger_reason,
            "Downstream Dragonfly timeout exceeded 3 consecutive probes"
        );
    }
}
