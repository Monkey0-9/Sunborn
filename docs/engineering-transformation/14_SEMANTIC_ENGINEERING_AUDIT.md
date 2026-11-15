# Semantic Engineering & Memory Audit

**Memory Vector Engine:** Qdrant Distributed HNSW  
**Point ID Mapping:** Deterministic UUID5 RFC 4122  
**Verification Guardrails:** Structural AST Analysis (Negation, Quantifier, Numeric)  
**Ambiguity Handling:** Top-8 Candidate Ambiguity Abstention  

---

## 1. Semantic Verification Pipeline

```mermaid
flowchart TD
    InQuery["Inbound Prompt Query"] --> Embed["Generate Vector Embedding (OpenAI / Local)"]
    Embed --> QdrantSearch["Qdrant Vector Search (Cosine Similarity)"]
    
    QdrantSearch --> TopK["Retrieve Top-8 Candidate Matches"]
    
    TopK --> AmbiguityCheck{"Ambiguity Check:<br/>|Sim(1) - Sim(2)| &le; 0.02?"}
    AmbiguityCheck -->|Yes: Ambiguous| Abstain["ABSTAIN: Fallback to Origin LLM"]
    
    AmbiguityCheck -->|No: Distinct Candidate| ASTGuard["Structural AST Guardrail Filter"]
    
    ASTGuard --> NegCheck{"Negation Shift?<br/>(e.g., 'not', 'never', 'deny')"}
    NegCheck -->|Detected| Reject1["REJECT: False-Positive Inversion"]
    
    NegCheck -->|Pass| QuantCheck{"Quantifier Shift?<br/>(e.g., 'all' vs 'none')"}
    QuantCheck -->|Detected| Reject2["REJECT: Semantic Inversion"]
    
    QuantCheck -->|Pass| NumCheck{"Numeric Transposition?<br/>(e.g., '$100M' vs '$10M')"}
    NumCheck -->|Detected| Reject3["REJECT: Factual Shift"]
    
    NumCheck -->|Pass| ConformalGate{"Conformal Risk Gate<br/>Non-Conformity S_i &le; Threshold"}
    ConformalGate -->|Pass| CacheHit["SAFE CACHE HIT: Authorize Reuse"]
    ConformalGate -->|Fail| Abstain
```

---

## 2. Elimination of Unsubstantiated Claims

The 5.2 baseline claimed to run an in-process 300M parameter BERT NLI cross-encoder. In reality, no neural network weights were packaged in the binary.
- **Architectural Honest Realignment:** The pipeline is explicitly branded and implemented as **Structural AST Guardrails** combined with **Top-8 Candidate Ambiguity Abstention**.
- **External Integration Trait:** For environments requiring genuine neural cross-encoder verification, `crates/sunborn-semantic/src/guardrails.rs` provides `HttpCrossEncoderBackend`, allowing clean offloading to an external Triton/vLLM inference server.
