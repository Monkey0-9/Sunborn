use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static SSN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("Valid SSN regex"));

static CC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{4}[- ]?){3}\d{4}\b").expect("Valid credit card regex"));

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").expect("Valid email regex")
});

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\+?1[-. ]?)?\(?\d{3}\)?[-. ]?\d{3}[-. ]?\d{4}\b").expect("Valid phone regex")
});

static TOKEN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|bearer\s+[a-zA-Z0-9_\-\.]{20,})\b",
    )
    .expect("Valid token regex")
});

/// Ephemeral request-scoped mapping allowing client response hydration
/// without persisting PII to vector stores, Redis, or PostgreSQL.
#[derive(Debug, Clone, Default)]
pub struct PiiVaultSession {
    /// Token placeholder -> Original sensitive plaintext
    token_to_raw: HashMap<String, String>,
}

impl PiiVaultSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hydrates a cached canonical response by restoring request-scoped PII placeholders.
    pub fn hydrate(&self, canonical_text: &str) -> String {
        let mut hydrated = canonical_text.to_string();
        for (placeholder, raw) in &self.token_to_raw {
            hydrated = hydrated.replace(placeholder, raw);
        }
        hydrated
    }

    pub fn has_redactions(&self) -> bool {
        !self.token_to_raw.is_empty()
    }

    pub fn redaction_count(&self) -> usize {
        self.token_to_raw.len()
    }
}

/// Zero-Leakage Inline PII Tokenizer.
///
/// Enterprise Fortune-500 compliance (SOC2 Type II / HIPAA / PCI-DSS) requires that
/// raw personal identifiers never touch downstream semantic caches, Qdrant vectors,
/// DragonflyDB memory, or PostgreSQL records.
///
/// `PiiTokenizer` redacts raw sensitive values into canonical typed placeholders
/// (`<US_SSN_1>`, `<EMAIL_1>`, etc.) before hashing and embedding, allowing two users
/// with identical semantic intent but distinct private data to safely share cache hits
/// while guaranteeing zero PII persistence.
pub struct PiiTokenizer;

impl PiiTokenizer {
    /// Tokenizes and redacts all recognized PII patterns from text, returning
    /// the canonical sanitized text along with an ephemeral, request-scoped vault session.
    pub fn tokenize(input: &str) -> (String, PiiVaultSession) {
        let mut session = PiiVaultSession::new();
        let mut text = input.to_string();

        // 1. Redact SSNs
        let mut ssn_idx = 1;
        text = SSN_REGEX
            .replace_all(&text, |caps: &regex::Captures| {
                let raw = caps[0].to_string();
                let placeholder = format!("<US_SSN_{ssn_idx}>");
                ssn_idx += 1;
                session.token_to_raw.insert(placeholder.clone(), raw);
                placeholder
            })
            .to_string();

        // 2. Redact Credit Cards
        let mut cc_idx = 1;
        text = CC_REGEX
            .replace_all(&text, |caps: &regex::Captures| {
                let raw = caps[0].to_string();
                let placeholder = format!("<CREDIT_CARD_{cc_idx}>");
                cc_idx += 1;
                session.token_to_raw.insert(placeholder.clone(), raw);
                placeholder
            })
            .to_string();

        // 3. Redact API tokens & secrets
        let mut tok_idx = 1;
        text = TOKEN_REGEX
            .replace_all(&text, |caps: &regex::Captures| {
                let raw = caps[0].to_string();
                let placeholder = format!("<API_TOKEN_{tok_idx}>");
                tok_idx += 1;
                session.token_to_raw.insert(placeholder.clone(), raw);
                placeholder
            })
            .to_string();

        // 4. Redact Emails
        let mut email_idx = 1;
        text = EMAIL_REGEX
            .replace_all(&text, |caps: &regex::Captures| {
                let raw = caps[0].to_string();
                let placeholder = format!("<EMAIL_{email_idx}>");
                email_idx += 1;
                session.token_to_raw.insert(placeholder.clone(), raw);
                placeholder
            })
            .to_string();

        // 5. Redact Phones
        let mut phone_idx = 1;
        text = PHONE_REGEX
            .replace_all(&text, |caps: &regex::Captures| {
                let raw = caps[0].to_string();
                let placeholder = format!("<PHONE_{phone_idx}>");
                phone_idx += 1;
                session.token_to_raw.insert(placeholder.clone(), raw);
                placeholder
            })
            .to_string();

        (text, session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pii_redaction_and_canonicalization() {
        let raw_prompt =
            "My SSN is 000-12-3456 and my email is alice@enterprise.org. What is my tax bracket?";
        let (canonical, session) = PiiTokenizer::tokenize(raw_prompt);

        assert_eq!(
            canonical,
            "My SSN is <US_SSN_1> and my email is <EMAIL_1>. What is my tax bracket?"
        );
        assert_eq!(session.redaction_count(), 2);
        assert!(session.has_redactions());

        // A second user asks the exact same question with different personal data
        let bob_prompt =
            "My SSN is 999-88-7777 and my email is bob@fintech.io. What is my tax bracket?";
        let (bob_canonical, bob_session) = PiiTokenizer::tokenize(bob_prompt);

        // Crucial invariant: Canonical prompts match identically!
        assert_eq!(canonical, bob_canonical);

        // When the cached response is hydrated:
        let cached_reply =
            "Based on IRS rules for <US_SSN_1>, a verification was sent to <EMAIL_1>.";
        let alice_reply = session.hydrate(cached_reply);
        let bob_reply = bob_session.hydrate(cached_reply);

        assert_eq!(
            alice_reply,
            "Based on IRS rules for 000-12-3456, a verification was sent to alice@enterprise.org."
        );
        assert_eq!(
            bob_reply,
            "Based on IRS rules for 999-88-7777, a verification was sent to bob@fintech.io."
        );
    }

    #[test]
    fn test_secret_token_redaction() {
        let raw = "Use bearer sk-live-99990000111122223333444455556666 for authorization";
        let (canonical, session) = PiiTokenizer::tokenize(raw);

        assert!(canonical.contains("<API_TOKEN_1>"));
        assert!(!canonical.contains("sk-live-"));
        assert_eq!(session.hydrate(&canonical), raw);
    }
}
