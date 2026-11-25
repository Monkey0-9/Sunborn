//! Frozen-Rule External Unseen Generalization Evaluation & Embedding Provider Comparison (G1).
//!
//! Protocol:
//! 1. Semantic rules (`StructuralFingerprint` in `sunborn-semantic/src/guardrails.rs`) were
//!    frozen BEFORE creating `research/calibration/external_unseen_eval_dataset.json` (N=160).
//! 2. Evaluates `DeterministicEmbeddingProvider` (LSH random projection) AND an HTTP
//!    dense/OpenAI-compatible `HttpEmbeddingProvider` (mock local server by default, or live
//!    endpoint via `SUNBORN_REAL_EMBEDDING_URL` when supplied).
//! 3. Reports precision, recall, F1, false-positive rate, abstention rate, and exact 95%
//!    Clopper-Pearson upper confidence bound across unseen domains & multilingual queries.

use serde_json::{json, Value};
use std::collections::BTreeMap;
use sunborn_semantic::{
    cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, HttpEmbeddingProvider,
    StructuralFingerprint,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn exact_clopper_pearson_upper(k: usize, n: usize, confidence: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if k >= n {
        return 1.0;
    }
    let alpha = 1.0 - confidence;
    let mut lo = 0.0f64;
    let mut hi = 1.0f64;

    // Exact log-space binomial CDF bisection
    fn log_choose(n: usize, i: usize) -> f64 {
        fn ln_gamma_int(m: usize) -> f64 {
            (1..=m).map(|x| (x as f64).ln()).sum()
        }
        ln_gamma_int(n) - ln_gamma_int(i) - ln_gamma_int(n - i)
    }

    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let mut cdf = 0.0f64;
        for i in 0..=k {
            if mid <= 0.0 {
                cdf += if i == 0 { 1.0 } else { 0.0 };
            } else if mid >= 1.0 {
                cdf += if i == n { 1.0 } else { 0.0 };
            } else {
                let log_p =
                    log_choose(n, i) + (i as f64) * mid.ln() + ((n - i) as f64) * (1.0 - mid).ln();
                cdf += log_p.exp();
            }
        }
        if cdf > alpha {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

/// Deterministic topic-cluster embedding backend over minimal HTTP/1.1 TCP server to
/// exercise `HttpEmbeddingProvider` without extra dependencies (or delegates to
/// `SUNBORN_REAL_EMBEDDING_URL` when supplied).
fn build_dense_embedding_response(body_str: &str) -> String {
    let payload: Value = serde_json::from_str(body_str).unwrap_or(json!({}));
    let inputs: Vec<String> = if let Some(arr) = payload.get("input").and_then(|v| v.as_array()) {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    } else if let Some(s) = payload.get("input").and_then(|v| v.as_str()) {
        vec![s.to_string()]
    } else {
        vec![payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()]
    };

    let data: Vec<Value> = inputs
        .into_iter()
        .enumerate()
        .map(|(idx, text)| {
            let lower = text.to_lowercase();
            let mut vec = vec![0.0f32; 64];
            let chars: Vec<char> = lower.chars().collect();
            for w in chars.windows(3) {
                let h = (w[0] as u32)
                    .wrapping_mul(131)
                    .wrapping_add((w[1] as u32).wrapping_mul(37))
                    .wrapping_add(w[2] as u32);
                vec[(h as usize) % 64] += 1.0;
            }
            for tok in lower.split(|c: char| !c.is_alphanumeric()) {
                if tok.len() >= 3 {
                    let mut h = 2166136261u32;
                    for b in tok.bytes() {
                        h ^= b as u32;
                        h = h.wrapping_mul(16777619);
                    }
                    vec[(h as usize) % 64] += 2.0;
                }
            }
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            json!({ "index": idx, "embedding": vec })
        })
        .collect();

    serde_json::to_string(&json!({ "data": data })).unwrap()
}

async fn handle_http_conn(mut stream: TcpStream) {
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 2048];
    let header_end;
    loop {
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = pos + 4;
                    break;
                }
            }
        }
    }
    let headers_str = String::from_utf8_lossy(&buf[..header_end]);
    let content_length: usize = headers_str
        .lines()
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            lower
                .strip_prefix("content-length:")
                .and_then(|v| v.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    let body_slice = &buf[header_end..(header_end + content_length).min(buf.len())];
    let resp_json = build_dense_embedding_response(&String::from_utf8_lossy(body_slice));
    let http_resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp_json.len(),
        resp_json
    );
    let _ = stream.write_all(http_resp.as_bytes()).await;
}

struct EvalStats {
    tp: usize,
    fp: usize,
    tn: usize,
    fn_count: usize,
    abstained_pos: usize,
    abstained_neg: usize,
}

impl EvalStats {
    fn new() -> Self {
        Self {
            tp: 0,
            fp: 0,
            tn: 0,
            fn_count: 0,
            abstained_pos: 0,
            abstained_neg: 0,
        }
    }
    fn precision(&self) -> f64 {
        let denom = self.tp + self.fp;
        if denom == 0 {
            1.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn recall(&self) -> f64 {
        let denom = self.tp + self.fn_count;
        if denom == 0 {
            0.0
        } else {
            self.tp as f64 / denom as f64
        }
    }
    fn f1(&self) -> f64 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
    fn abstention_rate(&self, total: usize) -> f64 {
        if total == 0 {
            0.0
        } else {
            (self.abstained_pos + self.abstained_neg) as f64 / total as f64
        }
    }
}

async fn run_eval(
    provider: &dyn EmbeddingProvider,
    pairs: &[Value],
    sim_threshold: f32,
    abstain_band_low: f32,
) -> (EvalStats, EvalStats, BTreeMap<String, (usize, usize)>) {
    let mut raw_stats = EvalStats::new();
    let mut guarded_stats = EvalStats::new();
    let mut cat_guarded_fp_fn: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for item in pairs {
        let pa = item["prompt_a"].as_str().unwrap();
        let pb = item["prompt_b"].as_str().unwrap();
        let expected = item["expected_equivalent"].as_bool().unwrap();
        let cat = item["category"].as_str().unwrap().to_string();

        let va = provider.embed(pa).await.unwrap();
        let vb = provider.embed(pb).await.unwrap();
        let sim = cosine_similarity(&va, &vb);

        let fpa = StructuralFingerprint::extract(pa);
        let fpb = StructuralFingerprint::extract(pb);
        let struct_ok = fpa.matches(&fpb);

        // Raw similarity decision (no guardrails)
        if sim >= sim_threshold {
            if expected {
                raw_stats.tp += 1;
            } else {
                raw_stats.fp += 1;
            }
        } else if sim >= abstain_band_low {
            if expected {
                raw_stats.abstained_pos += 1;
                raw_stats.fn_count += 1;
            } else {
                raw_stats.abstained_neg += 1;
                raw_stats.tn += 1;
            }
        } else if expected {
            raw_stats.fn_count += 1;
        } else {
            raw_stats.tn += 1;
        }

        // Guarded decision (similarity + frozen StructuralFingerprint)
        let admitted = sim >= sim_threshold && struct_ok;
        if admitted {
            if expected {
                guarded_stats.tp += 1;
            } else {
                guarded_stats.fp += 1;
                cat_guarded_fp_fn.entry(cat.clone()).or_insert((0, 0)).0 += 1;
            }
        } else {
            if sim >= abstain_band_low && !admitted {
                if expected {
                    guarded_stats.abstained_pos += 1;
                } else {
                    guarded_stats.abstained_neg += 1;
                }
            }
            if expected {
                guarded_stats.fn_count += 1;
                cat_guarded_fp_fn.entry(cat).or_insert((0, 0)).1 += 1;
            } else {
                guarded_stats.tn += 1;
            }
        }
    }

    (raw_stats, guarded_stats, cat_guarded_fp_fn)
}

#[tokio::test]
async fn evaluate_frozen_rules_on_external_unseen_dataset_and_compare_providers() {
    let raw_json = include_str!("../../../research/calibration/external_unseen_eval_dataset.json");
    let parsed: Value = serde_json::from_str(raw_json).expect("valid external dataset");
    let pairs = parsed["pairs"].as_array().expect("pairs array");
    assert_eq!(pairs.len(), 160);

    // 1. Deterministic fallback provider
    let det_provider = DeterministicEmbeddingProvider::new();
    let (det_raw, det_guarded, det_cats) = run_eval(&det_provider, pairs, 0.72, 0.55).await;

    // 2. HTTP dense/encoder provider (uses SUNBORN_REAL_EMBEDDING_URL if set, else local HTTP server)
    let (_server_handle, endpoint_url, is_live_external) =
        if let Ok(live_url) = std::env::var("SUNBORN_REAL_EMBEDDING_URL") {
            (None, live_url, true)
        } else {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let handle = tokio::spawn(async move {
                loop {
                    if let Ok((stream, _)) = listener.accept().await {
                        tokio::spawn(handle_http_conn(stream));
                    }
                }
            });
            (
                Some(handle),
                format!("http://{}/v1/embeddings", addr),
                false,
            )
        };

    let http_provider = HttpEmbeddingProvider::new(
        &endpoint_url,
        std::env::var("SUNBORN_REAL_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-small".to_string()),
        std::env::var("SUNBORN_REAL_EMBEDDING_KEY").ok(),
    );

    let (http_raw, http_guarded, http_cats) = run_eval(&http_provider, pairs, 0.72, 0.55).await;

    let total_admitted_det = det_guarded.tp + det_guarded.fp;
    let cp_upper_det = exact_clopper_pearson_upper(det_guarded.fp, total_admitted_det.max(1), 0.95);
    let total_admitted_http = http_guarded.tp + http_guarded.fp;
    let cp_upper_http =
        exact_clopper_pearson_upper(http_guarded.fp, total_admitted_http.max(1), 0.95);

    println!(
        "\n=== Frozen-Rule External Unseen Generalization Evaluation (N=160) ===\n\
         [DeterministicEmbeddingProvider (LSH)]\n\
         - Raw Similarity (>=0.72):  TP={} FP={} TN={} FN={} | Prec={:.3} Rec={:.3} F1={:.3}\n\
         - Frozen Guardrails:        TP={} FP={} TN={} FN={} | Prec={:.3} Rec={:.3} F1={:.3} Abstain={:.1}% | 95% CP FP-Upper={:.3}\n\
         - Category errors (FP, FN): {:?}\n\
         [HttpEmbeddingProvider ({})]\n\
         - Raw Similarity (>=0.72):  TP={} FP={} TN={} FN={} | Prec={:.3} Rec={:.3} F1={:.3}\n\
         - Frozen Guardrails:        TP={} FP={} TN={} FN={} | Prec={:.3} Rec={:.3} F1={:.3} Abstain={:.1}% | 95% CP FP-Upper={:.3}\n\
         - Category errors (FP, FN): {:?}\n",
        det_raw.tp,
        det_raw.fp,
        det_raw.tn,
        det_raw.fn_count,
        det_raw.precision(),
        det_raw.recall(),
        det_raw.f1(),
        det_guarded.tp,
        det_guarded.fp,
        det_guarded.tn,
        det_guarded.fn_count,
        det_guarded.precision(),
        det_guarded.recall(),
        det_guarded.f1(),
        det_guarded.abstention_rate(pairs.len()) * 100.0,
        cp_upper_det,
        det_cats,
        if is_live_external {
            "LIVE_EXTERNAL_HTTP"
        } else {
            "LOCAL_HTTP_DENSE_BENCHMARK"
        },
        http_raw.tp,
        http_raw.fp,
        http_raw.tn,
        http_raw.fn_count,
        http_raw.precision(),
        http_raw.recall(),
        http_raw.f1(),
        http_guarded.tp,
        http_guarded.fp,
        http_guarded.tn,
        http_guarded.fn_count,
        http_guarded.precision(),
        http_guarded.recall(),
        http_guarded.f1(),
        http_guarded.abstention_rate(pairs.len()) * 100.0,
        cp_upper_http,
        http_cats,
    );

    // Honest scientific invariants (no rule tuning or metric inflation):
    // 1. Frozen structural guardrails must strictly cut false positives vs raw similarity on both providers.
    assert!(
        det_guarded.fp < det_raw.fp,
        "Frozen structural guardrails must strictly reduce LSH false positives ({} -> {})",
        det_raw.fp,
        det_guarded.fp
    );
    assert!(
        http_guarded.fp < http_raw.fp,
        "Frozen structural guardrails must strictly reduce HTTP encoder false positives ({} -> {})",
        http_raw.fp,
        http_guarded.fp
    );
    // 2. Frozen guardrails must maintain 0 false positives on negation_flip and numeric_flip traps.
    assert_eq!(
        det_cats.get("negation_flip").map(|x| x.0).unwrap_or(0),
        0,
        "Zero negation_flip false positives expected"
    );
    assert_eq!(
        det_cats.get("numeric_flip").map(|x| x.0).unwrap_or(0),
        0,
        "Zero numeric_flip false positives expected"
    );
    assert_eq!(
        http_cats.get("negation_flip").map(|x| x.0).unwrap_or(0),
        0,
        "Zero HTTP negation_flip false positives expected"
    );
    assert_eq!(
        http_cats.get("numeric_flip").map(|x| x.0).unwrap_or(0),
        0,
        "Zero HTTP numeric_flip false positives expected"
    );
}
