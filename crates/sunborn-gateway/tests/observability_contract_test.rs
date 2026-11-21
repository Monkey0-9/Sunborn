//! Contract test for the observability surface.
//!
//! Two failures motivated this test:
//!
//! 1. The shipped alert rules referenced metric names the gateway never emits
//!    (`sunborn_vector_search_errors_total`, `sunborn_invalidation_dlq`,
//!    `sunborn_quota_rejections_total`, `sunborn_spend_ledger_errors_total`).
//!    Those rules could never fire, so the alerting story looked complete while
//!    being inert.
//! 2. Each rule points at a runbook anchor, and a renamed heading silently
//!    breaks that link.
//!
//! The test fails if an alert references a metric that a live scrape does not
//! contain, or a runbook anchor that no longer exists.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::collections::BTreeSet;
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

const ALERT_RULES: &str = include_str!("../../../deploy/k8s/prometheusrule.yaml");
const RUNBOOKS: &str = include_str!("../../../docs/runbooks/incident_playbooks.md");

/// Extract every `sunborn_*` token from the rule expressions, ignoring alert
/// names and annotation prose (which also mention metrics).
fn metric_names_referenced_by_alerts(yaml: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in yaml.lines() {
        let trimmed = line.trim_start();
        let Some(expr) = trimmed.strip_prefix("expr:") else {
            continue;
        };
        for token in expr.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if token.starts_with("sunborn_") {
                names.insert(token.to_string());
            }
        }
    }
    names
}

/// GitHub-style heading slug: lowercase, alphanumerics and hyphens kept,
/// whitespace becomes `-`, everything else dropped.
fn slugify(heading: &str) -> String {
    let mut out = String::new();
    for c in heading.trim().to_lowercase().chars() {
        if c.is_alphanumeric() || c == '-' {
            out.push(c);
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

fn runbook_anchors(markdown: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(slugify)
        .collect()
}

#[tokio::test]
async fn every_alert_metric_is_actually_emitted() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();

    let emitted: BTreeSet<String> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split('{').next())
        .filter_map(|l| l.split_whitespace().next())
        .map(|m| m.to_string())
        .collect();

    let referenced = metric_names_referenced_by_alerts(ALERT_RULES);
    assert!(
        !referenced.is_empty(),
        "failed to parse any expressions from the alert rules"
    );

    let missing: Vec<&String> = referenced
        .iter()
        .filter(|m| !emitted.contains(*m))
        .collect();
    assert!(
        missing.is_empty(),
        "alert rules reference metrics the gateway never emits: {missing:?}. \
         An alert on an absent metric can never fire. Emitted families: {emitted:?}"
    );
}

#[tokio::test]
async fn every_alert_runbook_link_resolves_to_a_heading() {
    let anchors = runbook_anchors(RUNBOOKS);
    let mut checked = 0;

    for line in ALERT_RULES.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("runbook_url:") else {
            continue;
        };
        let Some((_, anchor)) = rest.trim().trim_matches('"').split_once('#') else {
            panic!("runbook_url without an anchor, so the link lands at the top: {rest}");
        };
        assert!(
            anchors.contains(anchor),
            "alert runbook anchor '#{anchor}' has no matching '## ' heading in incident_playbooks.md. \
             Known anchors: {anchors:?}"
        );
        checked += 1;
    }

    assert!(checked > 0, "no runbook_url entries found in alert rules");
}

/// The one-hot circuit-breaker gauge is only useful if it tracks the breaker
/// rather than reporting a constant. Trip the breaker and require the sample to
/// move from `closed` to `open`.
#[tokio::test]
async fn circuit_breaker_gauge_reflects_live_state() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let scrape = |app: axum::Router| async move {
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
    };

    let sample = |body: &str, state_label: &str| -> u64 {
        let needle = format!("sunborn_cache_circuit_breaker_status{{state=\"{state_label}\"}}");
        body.lines()
            .find(|l| l.starts_with(&needle))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("missing sample for state={state_label}"))
    };

    let before = scrape(app.clone()).await;
    assert_eq!(sample(&before, "closed"), 1);
    assert_eq!(sample(&before, "open"), 0);

    state.circuit_breaker.trip().await;

    let after = scrape(app).await;
    assert_eq!(
        sample(&after, "open"),
        1,
        "gauge did not follow the breaker into OPEN"
    );
    assert_eq!(sample(&after, "closed"), 0);
}
