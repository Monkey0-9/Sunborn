use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use sunborn_core::{RequestEnvelope, Result, SunbornError};
use sunborn_gateway::origin::{
    ArbitrageProviderEntry, HttpLlmOriginProvider, LlmOriginProvider, MultiProviderArbitrageRouter,
    OriginExecutionResult,
};

struct MockProviderWithStatus {
    name: String,
    call_count: Arc<AtomicU32>,
    should_fail_with_status: Option<u16>,
}

#[async_trait]
impl LlmOriginProvider for MockProviderWithStatus {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if let Some(status) = self.should_fail_with_status {
            return Err(SunbornError::origin(format!(
                "Upstream provider {} failed with HTTP status {}",
                self.name, status
            )));
        }
        Ok(OriginExecutionResult {
            content: format!(
                "[Response from {} for model {}]: test completion",
                self.name, req.model
            ),
            prompt_tokens: 15,
            completion_tokens: 25,
            model: req.model.clone(),
            latency_ms: 12,
            finish_reason: Some("stop".to_string()),
            tool_calls: None,
        })
    }
}

#[tokio::test]
async fn test_grok_model_affinity_routing() {
    let grok_calls = Arc::new(AtomicU32::new(0));
    let openai_calls = Arc::new(AtomicU32::new(0));

    let grok_provider = Arc::new(MockProviderWithStatus {
        name: "xai-grok".to_string(),
        call_count: grok_calls.clone(),
        should_fail_with_status: None,
    });

    let openai_provider = Arc::new(MockProviderWithStatus {
        name: "openai-gpt".to_string(),
        call_count: openai_calls.clone(),
        should_fail_with_status: None,
    });

    let router = MultiProviderArbitrageRouter::new(vec![
        ArbitrageProviderEntry {
            name: "openai-gpt".to_string(),
            provider: openai_provider,
            cost_per_million_input: 5.0,
            cost_per_million_output: 15.0,
            priority: 1, // Higher priority default
        },
        ArbitrageProviderEntry {
            name: "xai-grok".to_string(),
            provider: grok_provider,
            cost_per_million_input: 3.0,
            cost_per_million_output: 10.0,
            priority: 2,
        },
    ]);

    // Request for grok-2 should prioritize the Grok provider despite lower base priority
    let req = RequestEnvelope::new(
        "tenant-test",
        "app-test",
        "grok-2-1212",
        "Explain non-exchangeable conformal risk control",
    );

    let res = router.execute(&req).await.unwrap();
    assert_eq!(res.model, "grok-2-1212");
    assert!(res.content.contains("xai-grok"));
    assert_eq!(grok_calls.load(Ordering::SeqCst), 1);
    assert_eq!(openai_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_multi_provider_sub_millisecond_failover_cascade() {
    let primary_calls = Arc::new(AtomicU32::new(0));
    let secondary_calls = Arc::new(AtomicU32::new(0));

    let primary_failing = Arc::new(MockProviderWithStatus {
        name: "anthropic-primary".to_string(),
        call_count: primary_calls.clone(),
        should_fail_with_status: Some(429), // Rate limited
    });

    let secondary_healthy = Arc::new(MockProviderWithStatus {
        name: "google-gemini-secondary".to_string(),
        call_count: secondary_calls.clone(),
        should_fail_with_status: None,
    });

    let router = MultiProviderArbitrageRouter::new(vec![
        ArbitrageProviderEntry {
            name: "anthropic-primary".to_string(),
            provider: primary_failing,
            cost_per_million_input: 3.0,
            cost_per_million_output: 15.0,
            priority: 1,
        },
        ArbitrageProviderEntry {
            name: "google-gemini-secondary".to_string(),
            provider: secondary_healthy,
            cost_per_million_input: 2.5,
            cost_per_million_output: 10.0,
            priority: 2,
        },
    ]);

    let req = RequestEnvelope::new(
        "tenant-test",
        "app-test",
        "claude-3-5-sonnet",
        "Synthesize quarterly financial statements",
    );

    let start = std::time::Instant::now();
    let res = router.execute(&req).await.unwrap();
    let elapsed = start.elapsed();

    // Primary was attempted and failed
    assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
    // Secondary seamlessly took over
    assert_eq!(secondary_calls.load(Ordering::SeqCst), 1);
    assert!(res.content.contains("google-gemini-secondary"));
    assert!(
        elapsed.as_millis() < 50,
        "In-process failover took {:?}, expected < 50ms",
        elapsed
    );
}

#[test]
fn test_grok_factory_constructor() {
    let client = HttpLlmOriginProvider::for_grok(Some("xai-test-key".to_string()));
    assert_eq!(client.endpoint_url, "https://api.x.ai/v1");
    assert_eq!(client.api_key.as_deref(), Some("xai-test-key"));
}
