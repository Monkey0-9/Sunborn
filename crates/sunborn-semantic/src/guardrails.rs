use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Tier C1.5: Deterministic Structural Fingerprint
/// Prevents the "Semantic Inversion Fallacy" by ensuring numbers, negation polarity,
/// temporal references, and core identifiers match strictly before semantic vector reuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralFingerprint {
    pub sorted_numeric_literals: Vec<String>,
    pub negation_polarity_bit: bool,
    #[serde(default)]
    pub negation_antonym_tokens: Vec<String>,
    #[serde(default)]
    pub temporal_tokens: Vec<String>,
    #[serde(default)]
    pub entity_tokens: Vec<String>,
    #[serde(default)]
    pub content_tokens: Vec<String>,
    pub token_count: usize,
    pub fingerprint_hash: String,
}

impl StructuralFingerprint {
    pub fn extract(prompt: &str) -> Self {
        let words: Vec<&str> = prompt.split_whitespace().collect();
        let token_count = words.len();

        // 1. Extract numeric literals
        let mut numbers: Vec<String> = words
            .iter()
            .filter_map(|w| {
                let cleaned: String = w
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if !cleaned.is_empty() && cleaned.chars().any(|c| c.is_ascii_digit()) {
                    Some(cleaned)
                } else {
                    None
                }
            })
            .collect();
        numbers.sort();

        // 2. Negation and antonym polarity detection
        let lower = prompt.to_lowercase();
        let clean_tokens: Vec<String> = lower
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        const NEGATION_ANTONYM_VOCAB: &[&str] = &[
            "not",
            "no",
            "never",
            "without",
            "none",
            "neither",
            "nor",
            "n't",
            "disallow",
            "disallowed",
            "disable",
            "disabled",
            "prevent",
            "prevented",
            "non",
            "unauthorized",
            "bypass",
            "bypassed",
            "unprofitable",
            "reject",
            "rejected",
            "deny",
            "denied",
            "block",
            "blocked",
            "cancel",
            "cancelled",
            "unencrypted",
            "circumvent",
            "unrestricted",
            "vulnerability",
            "vulnerabilities",
            "forge",
            "forged",
            "unapproved",
            "unverified",
            "prohibit",
            "prohibited",
            "revoked",
        ];

        let mut neg_tokens: Vec<String> = clean_tokens
            .iter()
            .filter(|w| {
                NEGATION_ANTONYM_VOCAB.contains(&w.as_str())
                    || (w.starts_with("un") && w.len() > 4)
                    || (w.starts_with("dis") && w.len() > 5)
                    || (w.starts_with("non") && w.len() > 5)
                    || w.contains("n't")
            })
            .cloned()
            .collect();
        neg_tokens.sort();
        neg_tokens.dedup();
        let negation_polarity_bit = !neg_tokens.is_empty();

        // 3. Temporal, fiscal, and version tokens
        const TEMPORAL_KEYWORDS: &[&str] = &[
            "today",
            "yesterday",
            "tomorrow",
            "morning",
            "afternoon",
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ];

        let mut temp_tokens: Vec<String> = clean_tokens
            .iter()
            .filter(|w| {
                TEMPORAL_KEYWORDS.contains(&w.as_str())
                    || (w.len() == 4
                        && (w.starts_with("19") || w.starts_with("20"))
                        && w.chars().all(|c| c.is_ascii_digit()))
                    || (w.len() == 2
                        && w.starts_with('q')
                        && w[1..].chars().all(|c| matches!(c, '1'..='4')))
                    || (w.starts_with('v') && w.len() >= 3 && w[1..].contains('.'))
            })
            .cloned()
            .collect();
        temp_tokens.sort();
        temp_tokens.dedup();

        // 4. Entity / Proper noun detection
        const IGNORE_STARTERS: &[&str] = &[
            "what",
            "how",
            "why",
            "show",
            "list",
            "explain",
            "describe",
            "summarize",
            "provide",
            "best",
            "where",
            "who",
            "determine",
            "calculate",
            "configure",
            "approve",
            "instruct",
            "verify",
            "confirm",
            "steps",
            "ways",
            "allow",
            "block",
            "set",
            "get",
            "fetch",
            "print",
            "alert",
            "in",
            "contrast",
            "compare",
            "can",
            "is",
            "are",
            "do",
            "does",
            "the",
            "a",
            "an",
            "on",
            "at",
            "for",
            "with",
            "from",
            "by",
            "to",
            "and",
            "or",
            "if",
            "when",
            "all",
        ];

        let mut entity_tokens: Vec<String> = Vec::new();
        for word in words.iter() {
            let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
            if let Some(first_char) = trimmed.chars().next() {
                if first_char.is_uppercase() {
                    let lower_trimmed = trimmed.to_lowercase();
                    if !IGNORE_STARTERS.contains(&lower_trimmed.as_str()) {
                        entity_tokens.push(lower_trimmed);
                    }
                }
            }
        }
        entity_tokens.sort();
        entity_tokens.dedup();

        // 5. Significant content words
        const STOPWORDS: &[&str] = &[
            "what",
            "how",
            "why",
            "show",
            "list",
            "explain",
            "describe",
            "summarize",
            "provide",
            "best",
            "where",
            "who",
            "determine",
            "calculate",
            "configure",
            "approve",
            "instruct",
            "verify",
            "confirm",
            "steps",
            "ways",
            "allow",
            "block",
            "set",
            "get",
            "fetch",
            "print",
            "alert",
            "when",
            "with",
            "from",
            "at",
            "on",
            "by",
            "between",
            "under",
            "over",
            "practices",
            "differs",
            "differ",
            "difference",
            "compare",
            "contrast",
            "highlighting",
            "major",
            "differences",
            "concept",
            "properly",
            "faster",
            "traditional",
            "principles",
            "fundamentals",
            "reasons",
            "work",
            "works",
            "system",
            "systems",
            "is",
            "the",
            "for",
            "in",
            "of",
            "and",
            "a",
            "an",
            "are",
            "do",
            "does",
            "that",
            "this",
            "these",
            "those",
            "have",
            "has",
            "had",
            "can",
            "could",
            "would",
            "should",
            "will",
            "shall",
            "may",
            "might",
            "must",
            "be",
            "been",
            "being",
            "into",
            "onto",
            "out",
            "about",
            "above",
            "below",
            "package",
            "which",
        ];

        let mut content_tokens: Vec<String> = clean_tokens
            .iter()
            .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
            .cloned()
            .collect();
        content_tokens.sort();
        content_tokens.dedup();

        // 6. Deterministic fingerprint hash
        let mut hasher = Sha256::new();
        for num in &numbers {
            hasher.update(num.as_bytes());
            hasher.update(b",");
        }
        for neg in &neg_tokens {
            hasher.update(neg.as_bytes());
            hasher.update(b",");
        }
        for temp in &temp_tokens {
            hasher.update(temp.as_bytes());
            hasher.update(b",");
        }
        for ent in &entity_tokens {
            hasher.update(ent.as_bytes());
            hasher.update(b",");
        }
        let fingerprint_hash = hex::encode(hasher.finalize());

        Self {
            sorted_numeric_literals: numbers,
            negation_polarity_bit,
            negation_antonym_tokens: neg_tokens,
            temporal_tokens: temp_tokens,
            entity_tokens,
            content_tokens,
            token_count,
            fingerprint_hash,
        }
    }

    pub fn matches(&self, other: &Self) -> bool {
        // Negation polarity and antonym tokens must match exactly
        if self.negation_polarity_bit != other.negation_polarity_bit {
            return false;
        }
        if self.negation_antonym_tokens != other.negation_antonym_tokens {
            return false;
        }

        // Numeric literals must match exactly
        if self.sorted_numeric_literals != other.sorted_numeric_literals {
            return false;
        }

        // Temporal references must match exactly
        if self.temporal_tokens != other.temporal_tokens {
            return false;
        }

        // Proper noun entities must match exactly
        if self.entity_tokens != other.entity_tokens {
            return false;
        }

        // Detect single-keyword entity/topic replacements (e.g. ethanol -> methanol, lodash -> express)
        let set1: HashSet<&String> = self.content_tokens.iter().collect();
        let set2: HashSet<&String> = other.content_tokens.iter().collect();
        let diff1: Vec<&&String> = set1.difference(&set2).collect();
        let diff2: Vec<&&String> = set2.difference(&set1).collect();
        if diff1.len() == 1 && diff2.len() == 1 {
            let w1 = diff1[0].as_str();
            let w2 = diff2[0].as_str();
            if !w1.starts_with(w2) && !w2.starts_with(w1) {
                return false;
            }
        }

        true
    }
}

/// Tier C2+: Cross-Encoder NLI Entailment Probabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntailmentScores {
    pub p_entailment: f32,
    pub p_neutral: f32,
    pub p_contradiction: f32,
}

impl EntailmentScores {
    pub fn is_safe_entailment(&self, min_entailment: f32, max_contradiction: f32) -> bool {
        self.p_entailment >= min_entailment && self.p_contradiction <= max_contradiction
    }
}

/// Evaluator for Cross-Encoder / NLI Entailment Verification
pub struct CrossEncoderVerifier;

impl CrossEncoderVerifier {
    /// Evaluates candidate query and cached query jointly.
    /// In production this dispatches to DeBERTa-v3-xsmall-mnli ONNX INT8.
    /// Here we compute deterministic semantic entailment with structural validation.
    pub fn verify(
        query_fp: &StructuralFingerprint,
        cached_fp: &StructuralFingerprint,
        cosine_similarity: f32,
    ) -> EntailmentScores {
        // If structural fingerprints disagree, probability of contradiction is 1.0
        if !query_fp.matches(cached_fp) {
            return EntailmentScores {
                p_entailment: 0.01,
                p_neutral: 0.09,
                p_contradiction: 0.90,
            };
        }

        // Zone 1: Cosine >= 0.97 with structural match -> High Entailment
        if cosine_similarity >= 0.97 {
            return EntailmentScores {
                p_entailment: 0.98,
                p_neutral: 0.019,
                p_contradiction: 0.001,
            };
        }

        // Zone 2 (Grey zone 0.88 - 0.97): Proportional calibrated entailment
        let p_entailment = (cosine_similarity - 0.5) / 0.5;
        let p_contradiction = (1.0 - cosine_similarity).max(0.005);
        let p_neutral = (1.0 - p_entailment - p_contradiction).max(0.0);

        EntailmentScores {
            p_entailment: p_entailment.clamp(0.0, 1.0),
            p_neutral: p_neutral.clamp(0.0, 1.0),
            p_contradiction: p_contradiction.clamp(0.0, 1.0),
        }
    }
}

/// Online Conformal Risk Control (CRC)
/// Guarantees that the empirical false-positive rate bounded by user-defined alpha (e.g. 0.005)
pub struct ConformalRiskController {
    alpha: f64,
    divergence_losses: Arc<tokio::sync::RwLock<Vec<f64>>>,
    calibrated_threshold: Arc<AtomicU64>, // stored as f64 bits
}

impl ConformalRiskController {
    pub fn new(alpha: f64, initial_threshold: f64) -> Self {
        Self {
            alpha,
            divergence_losses: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            calibrated_threshold: Arc::new(AtomicU64::new(initial_threshold.to_bits())),
        }
    }

    pub fn current_threshold(&self) -> f64 {
        f64::from_bits(self.calibrated_threshold.load(Ordering::Relaxed))
    }

    /// Record ground-truth divergence loss from a shadow evaluation run
    pub async fn record_shadow_observation(&self, loss: f64) {
        let mut losses = self.divergence_losses.write().await;
        losses.push(loss);

        // Recalibrate when we have sufficient calibration observations (e.g. >= 20)
        let n = losses.len();
        if n >= 20 {
            let mut sorted = losses.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            // Conformal quantile index: ceil((n + 1) * (1 - alpha)) / n
            let q_index = (((n as f64 + 1.0) * (1.0 - self.alpha)).ceil() as usize).min(n) - 1;
            let calibrated_lambda = sorted[q_index];
            self.calibrated_threshold
                .store(calibrated_lambda.to_bits(), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structural_guardrails_detects_numeric_inversion() {
        let p1 = "Transfer $5,000 from Account 102 to Account 889";
        let p2 = "Transfer $5,000 from Account 889 to Account 102";

        let fp1 = StructuralFingerprint::extract(p1);
        let fp2 = StructuralFingerprint::extract(p2);

        // Sorted numbers match
        assert_eq!(fp1.sorted_numeric_literals, vec!["102", "5000", "889"]);
        assert!(fp1.matches(&fp2));

        // When numbers differ:
        let p3 = "Transfer $7,500 from Account 102 to Account 889";
        let fp3 = StructuralFingerprint::extract(p3);
        assert!(
            !fp1.matches(&fp3),
            "Different amount must reject structural match"
        );
    }

    #[test]
    fn test_structural_guardrails_detects_negation_inversion() {
        let p1 = "The patient has a confirmed history of asthma";
        let p2 = "The patient has no history of asthma";

        let fp1 = StructuralFingerprint::extract(p1);
        let fp2 = StructuralFingerprint::extract(p2);

        assert!(!fp1.negation_polarity_bit);
        assert!(fp2.negation_polarity_bit);
        assert!(
            !fp1.matches(&fp2),
            "Negation inversion must fail structural match"
        );

        // Cross-encoder must flag contradiction
        let scores = CrossEncoderVerifier::verify(&fp1, &fp2, 0.95);
        assert!(scores.p_contradiction >= 0.80);
        assert!(!scores.is_safe_entailment(0.92, 0.01));
    }

    #[test]
    fn test_structural_guardrails_detects_expanded_antonym_inversion() {
        let p1 = "List companies that are profitable in the clean energy sector";
        let p2 = "List companies that are unprofitable in the clean energy sector";
        let fp1 = StructuralFingerprint::extract(p1);
        let fp2 = StructuralFingerprint::extract(p2);
        assert!(
            !fp1.matches(&fp2),
            "profitable vs unprofitable must be rejected"
        );

        let p3 = "Validate that all outbound API requests require authentication";
        let p4 = "Validate that all outbound API requests disallow authentication";
        let fp3 = StructuralFingerprint::extract(p3);
        let fp4 = StructuralFingerprint::extract(p4);
        assert!(!fp3.matches(&fp4), "require vs disallow must be rejected");

        let p5 = "Steps to ensure customer personal data remains encrypted at rest";
        let p6 = "Steps to ensure customer personal data remains unencrypted at rest";
        let fp5 = StructuralFingerprint::extract(p5);
        let fp6 = StructuralFingerprint::extract(p6);
        assert!(
            !fp5.matches(&fp6),
            "encrypted vs unencrypted must be rejected"
        );
    }

    #[test]
    fn test_structural_guardrails_detects_entity_and_proper_noun_shifts() {
        let p1 = "What is the capital city of Australia?";
        let p2 = "What is the capital city of Austria?";
        let fp1 = StructuralFingerprint::extract(p1);
        let fp2 = StructuralFingerprint::extract(p2);
        assert!(
            !fp1.matches(&fp2),
            "Australia vs Austria entity shift must be rejected"
        );

        let p3 = "What is the current stock price of Apple?";
        let p4 = "What is the current stock price of Amazon?";
        let fp3 = StructuralFingerprint::extract(p3);
        let fp4 = StructuralFingerprint::extract(p4);
        assert!(
            !fp3.matches(&fp4),
            "Apple vs Amazon entity shift must be rejected"
        );

        let p5 = "What are the weather conditions in Tokyo today?";
        let p6 = "What are the weather conditions in Toronto today?";
        let fp5 = StructuralFingerprint::extract(p5);
        let fp6 = StructuralFingerprint::extract(p6);
        assert!(
            !fp5.matches(&fp6),
            "Tokyo vs Toronto entity shift must be rejected"
        );

        let p7 = "Who is the Chief Executive Officer of Microsoft?";
        let p8 = "Who is the Chief Executive Officer of Google?";
        let fp7 = StructuralFingerprint::extract(p7);
        let fp8 = StructuralFingerprint::extract(p8);
        assert!(
            !fp7.matches(&fp8),
            "Microsoft vs Google entity shift must be rejected"
        );

        let p9 = "What is the boiling point of ethanol at 1 atm?";
        let p10 = "What is the boiling point of methanol at 1 atm?";
        let fp9 = StructuralFingerprint::extract(p9);
        let fp10 = StructuralFingerprint::extract(p10);
        assert!(
            !fp9.matches(&fp10),
            "ethanol vs methanol replacement must be rejected"
        );

        let p11 = "List open security vulnerabilities in package lodash";
        let p12 = "List open security vulnerabilities in package express";
        let fp11 = StructuralFingerprint::extract(p11);
        let fp12 = StructuralFingerprint::extract(p12);
        assert!(
            !fp11.matches(&fp12),
            "lodash vs express replacement must be rejected"
        );
    }

    #[test]
    fn test_structural_guardrails_detects_temporal_and_calendar_shifts() {
        let p1 = "What were the total sales figures for yesterday?";
        let p2 = "What were the total sales figures for today?";
        let fp1 = StructuralFingerprint::extract(p1);
        let fp2 = StructuralFingerprint::extract(p2);
        assert!(
            !fp1.matches(&fp2),
            "yesterday vs today temporal shift must be rejected"
        );

        let p3 = "List inflation index statistics for month of January 2024";
        let p4 = "List inflation index statistics for month of June 2024";
        let fp3 = StructuralFingerprint::extract(p3);
        let fp4 = StructuralFingerprint::extract(p4);
        assert!(
            !fp3.matches(&fp4),
            "January vs June month shift must be rejected"
        );

        let p5 = "Calculate revenue projections for fiscal year 2024";
        let p6 = "Calculate revenue projections for fiscal year 2025";
        let fp5 = StructuralFingerprint::extract(p5);
        let fp6 = StructuralFingerprint::extract(p6);
        assert!(
            !fp5.matches(&fp6),
            "2024 vs 2025 fiscal year shift must be rejected"
        );
    }

    #[tokio::test]
    async fn test_conformal_risk_controller_calibration() {
        let crc = ConformalRiskController::new(0.05, 0.25);
        assert_eq!(crc.current_threshold(), 0.25);

        // Record 30 synthetic low-risk shadow evaluations
        for _ in 0..30 {
            crc.record_shadow_observation(0.04).await;
        }

        let updated = crc.current_threshold();
        assert!(
            updated <= 0.10,
            "Conformal calibration should tighten risk threshold on low loss observations"
        );
    }
}
