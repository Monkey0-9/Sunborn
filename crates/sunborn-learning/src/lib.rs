pub mod drift;
pub mod observation;
pub mod policy;
pub mod shadow_evaluator;

pub use drift::{DriftDetector, WorkloadDistribution};
pub use observation::{ExecutionRecord, TelemetryWindow};
pub use policy::{CachePolicy, PolicyLifecycleState};
pub use shadow_evaluator::{SafetyGate, ShadowEvaluationResult, ShadowEvaluator};
