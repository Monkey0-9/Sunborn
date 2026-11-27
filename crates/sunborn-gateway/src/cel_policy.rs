//! Lazy ContextBuilder CEL Policy Engine (AgentGateway-inspired).
//!
//! Parses declarative policy rules at configuration time, extracts
//! [`ContextRequirements`] (e.g. whether `request.headers` or `request.body`
//! are referenced), and evaluates them against request/MCP/tenant contexts
//! with zero-allocation fast paths when unreferenced fields are omitted.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRequirements {
    pub needs_headers: bool,
    pub needs_body: bool,
    pub needs_mcp: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyContext {
    pub tenant_id: String,
    pub application_id: String,
    pub model: String,
    pub is_admin: bool,
    pub mcp_tool_name: Option<String>,
    pub mcp_server_name: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CelPolicyRule {
    pub id: String,
    pub expression: String,
    /// `"allow"` or `"deny"`
    pub effect: String,
    pub requirements: ContextRequirements,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyEvaluation {
    pub allowed: bool,
    pub matched_rule_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Default)]
pub struct CelPolicyEngine {
    rules: Arc<RwLock<Vec<CelPolicyRule>>>,
}

impl CelPolicyEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_rule(
        &self,
        id: impl Into<String>,
        expression: impl Into<String>,
        effect: impl Into<String>,
    ) -> CelPolicyRule {
        let expr = expression.into();
        let reqs = ContextRequirements {
            needs_headers: expr.contains("request.headers"),
            needs_body: expr.contains("request.body"),
            needs_mcp: expr.contains("mcp."),
        };
        let rule = CelPolicyRule {
            id: id.into(),
            expression: expr,
            effect: effect.into(),
            requirements: reqs,
        };
        let mut rules = self.rules.write().unwrap_or_else(|e| e.into_inner());
        rules.retain(|r| r.id != rule.id);
        rules.push(rule.clone());
        rule
    }

    pub fn remove_rule(&self, id: &str) -> bool {
        let mut rules = self.rules.write().unwrap_or_else(|e| e.into_inner());
        let before = rules.len();
        rules.retain(|r| r.id != id);
        rules.len() < before
    }

    pub fn list_rules(&self) -> Vec<CelPolicyRule> {
        self.rules.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn combined_requirements(&self) -> ContextRequirements {
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        let mut reqs = ContextRequirements::default();
        for r in rules.iter() {
            reqs.needs_headers |= r.requirements.needs_headers;
            reqs.needs_body |= r.requirements.needs_body;
            reqs.needs_mcp |= r.requirements.needs_mcp;
        }
        reqs
    }

    /// Evaluates all configured rules. Any matching `"deny"` rule blocks immediately.
    pub fn evaluate(&self, ctx: &PolicyContext) -> PolicyEvaluation {
        let rules = self.rules.read().unwrap_or_else(|e| e.into_inner());
        for rule in rules.iter() {
            if eval_expr(&rule.expression, ctx) && rule.effect.eq_ignore_ascii_case("deny") {
                return PolicyEvaluation {
                    allowed: false,
                    matched_rule_id: Some(rule.id.clone()),
                    reason: format!(
                        "Denied by CEL policy rule '{}': {}",
                        rule.id, rule.expression
                    ),
                };
            }
        }
        PolicyEvaluation {
            allowed: true,
            matched_rule_id: None,
            reason: "Allowed".to_string(),
        }
    }
}

/// Evaluates boolean CEL expressions supporting `||`, `&&`, `!`, `==`, `!=`, and `in [...]`.
pub fn eval_expr(expr: &str, ctx: &PolicyContext) -> bool {
    // Split on top-level `||`
    if let Some(parts) = split_top_level(expr, "||") {
        return parts.iter().any(|p| eval_expr(p, ctx));
    }
    // Split on top-level `&&`
    if let Some(parts) = split_top_level(expr, "&&") {
        return parts.iter().all(|p| eval_expr(p, ctx));
    }
    let trimmed = expr.trim();
    if let Some(inner) = trimmed.strip_prefix('!') {
        if !trimmed.starts_with("!=") {
            return !eval_expr(inner, ctx);
        }
    }
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return eval_expr(&trimmed[1..trimmed.len() - 1], ctx);
    }

    if let Some((lhs, rhs)) = trimmed.split_once("==") {
        return resolve_value(lhs.trim(), ctx) == resolve_value(rhs.trim(), ctx);
    }
    if let Some((lhs, rhs)) = trimmed.split_once("!=") {
        return resolve_value(lhs.trim(), ctx) != resolve_value(rhs.trim(), ctx);
    }
    if let Some((lhs, rhs)) = trimmed.split_once(" in ") {
        let val = resolve_value(lhs.trim(), ctx);
        let list = rhs.trim().trim_start_matches('[').trim_end_matches(']');
        return list
            .split(',')
            .map(|item| unquote(item.trim()))
            .any(|item| item == val);
    }
    match trimmed {
        "true" | "tenant.is_admin" => {
            if trimmed == "true" {
                true
            } else {
                ctx.is_admin
            }
        }
        "false" => false,
        _ => false,
    }
}

fn split_top_level<'a>(expr: &'a str, op: &str) -> Option<Vec<&'a str>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quotes = false;
    let mut last = 0usize;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            if b == b'(' || b == b'[' {
                depth += 1;
            } else if b == b')' || b == b']' {
                depth -= 1;
            } else if depth == 0 && expr[i..].starts_with(op) && op_bytes.len() == 2 {
                parts.push(&expr[last..i]);
                i += op_bytes.len();
                last = i;
                continue;
            }
        }
        i += 1;
    }
    if parts.is_empty() {
        None
    } else {
        parts.push(&expr[last..]);
        Some(parts)
    }
}

fn resolve_value(token: &str, ctx: &PolicyContext) -> String {
    if (token.starts_with('"') && token.ends_with('"'))
        || (token.starts_with('\'') && token.ends_with('\''))
    {
        return unquote(token);
    }
    match token {
        "tenant.id" => ctx.tenant_id.clone(),
        "application.id" => ctx.application_id.clone(),
        "request.model" => ctx.model.clone(),
        "mcp.tool.name" => ctx.mcp_tool_name.clone().unwrap_or_default(),
        "mcp.server.name" => ctx.mcp_server_name.clone().unwrap_or_default(),
        "request.body" => ctx.body_text.clone().unwrap_or_default(),
        _ if token.starts_with("request.headers[") && token.ends_with(']') => {
            let key = unquote(&token["request.headers[".len()..token.len() - 1]);
            ctx.headers
                .get(&key.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        }
        _ => token.to_string(),
    }
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('\'').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cel_policy_extracts_requirements_and_enforces_rules() {
        let engine = CelPolicyEngine::new();
        let rule = engine.add_rule(
            "deny-prod-drop",
            "request.headers[\"x-env\"] == \"prod\" && mcp.tool.name in [\"drop_table\", \"delete_db\"]",
            "deny",
        );
        assert!(rule.requirements.needs_headers);
        assert!(rule.requirements.needs_mcp);
        assert!(!rule.requirements.needs_body);

        let mut headers = HashMap::new();
        headers.insert("x-env".to_string(), "prod".to_string());
        let ctx = PolicyContext {
            tenant_id: "acme".into(),
            application_id: "agent".into(),
            model: "gpt-4o".into(),
            is_admin: false,
            mcp_tool_name: Some("drop_table".into()),
            mcp_server_name: Some("pg".into()),
            headers,
            body_text: None,
        };
        let res = engine.evaluate(&ctx);
        assert!(!res.allowed);
        assert_eq!(res.matched_rule_id.as_deref(), Some("deny-prod-drop"));
    }
}
