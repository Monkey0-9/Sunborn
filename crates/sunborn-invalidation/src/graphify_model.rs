//! # Graphify Multi-Relational Knowledge Model
//!
//! Inspired by the deterministic AST and multi-modal knowledge graph architecture
//! of Graphify (Graphify-Labs/graphify). Defines typed node entities, typed directional
//! dependency relations, confidence annotations, and causal provenance metadata.

use serde::{Deserialize, Serialize};

/// Classification of node entities within the multi-relational knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphifyNodeType {
    /// Source code file or asset
    File,
    /// Function, method, or callable procedure
    Function,
    /// Class, struct, trait, or interface declaration
    Class,
    /// Module, package, or namespace
    Module,
    /// Markdown, documentation, or specification document
    Document,
    /// Section, heading, or chunk within a document
    Section,
    /// Database table, view, or collection schema
    Table,
    /// LLM prompt template or system instruction
    PromptTemplate,
    /// In-memory or persistent cache record entry
    CacheEntry,
    /// Generative or embedding model definition
    Model,
}

impl GraphifyNodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Function => "function",
            Self::Class => "class",
            Self::Module => "module",
            Self::Document => "document",
            Self::Section => "section",
            Self::Table => "table",
            Self::PromptTemplate => "prompt_template",
            Self::CacheEntry => "cache_entry",
            Self::Model => "model",
        }
    }
}

/// Typed directional relationship between two nodes in the knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphifyRelation {
    /// Direct function or method invocation: caller -> callee
    Calls,
    /// Indirect or dynamic dispatch invocation: caller -> callee
    IndirectCall,
    /// Code or documentation referencing a symbol or type: reader -> target
    References,
    /// File or module importing another asset: importer -> imported
    Imports,
    /// Specific symbol import: importer -> imported_symbol
    ImportsFrom,
    /// Dynamic runtime import: importer -> imported
    DynamicImport,
    /// Module re-exporting a symbol: exporter -> symbol
    ReExports,
    /// File or class defining a symbol: container -> member
    Defines,
    /// Subclass inheriting from a base class: derived -> base
    Inherits,
    /// Type extending an existing type: derived -> base
    Extends,
    /// Type implementing an interface or trait: implementer -> interface
    Implements,
    /// Generic usage or composition link: user -> used
    Uses,
    /// Function or query reading from a table or state: reader -> table
    ReadsFrom,
    /// Function or mutation writing to a table or state: writer -> table
    WritesTo,
    /// Causal dependency link: child -> parent dependency
    DependsOn,
}

impl GraphifyRelation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::IndirectCall => "indirect_call",
            Self::References => "references",
            Self::Imports => "imports",
            Self::ImportsFrom => "imports_from",
            Self::DynamicImport => "dynamic_import",
            Self::ReExports => "re_exports",
            Self::Defines => "defines",
            Self::Inherits => "inherits",
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::Uses => "uses",
            Self::ReadsFrom => "reads_from",
            Self::WritesTo => "writes_to",
            Self::DependsOn => "depends_on",
        }
    }

    /// Returns true if this relation is included in the default blast-radius traversal.
    pub fn is_affected_by_default(&self) -> bool {
        matches!(
            self,
            Self::Calls
                | Self::IndirectCall
                | Self::References
                | Self::Imports
                | Self::ImportsFrom
                | Self::DynamicImport
                | Self::ReExports
                | Self::Defines
                | Self::Inherits
                | Self::Extends
                | Self::Implements
                | Self::Uses
                | Self::ReadsFrom
                | Self::WritesTo
                | Self::DependsOn
        )
    }
}

/// Confidence rating of an extracted knowledge graph relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphifyConfidence {
    /// Deterministic extraction from AST, compiler symbols, or verified schemas
    Extracted,
    /// Inferred relationship from semantic embeddings, LLMs, or heuristic analysis
    Inferred,
}

/// Represents an individual entity within the multi-relational knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphifyNode {
    /// Unique string identifier (e.g. "src/auth/service.rs::login" or "cache::prompt_hash_123")
    pub id: String,
    /// Numeric identifier for bitset indexing and Roaring Treemap acceleration
    pub numeric_id: u64,
    /// Human-readable entity label or symbol name
    pub label: String,
    /// Classified entity type
    pub node_type: GraphifyNodeType,
    /// Path to the source file or document where defined, if applicable
    pub source_file: Option<String>,
    /// Source line number or symbol location string (e.g. "L120" or "L120-L145")
    pub source_location: Option<String>,
    /// 64-bit content-addressed SHA-256 digest of the entity content
    pub content_hash: Option<u64>,
    /// Community cluster identifier derived via modularity / label propagation
    pub community_id: Option<u32>,
    /// Flexible metadata payload for FinOps, volatility, and domain attributes
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl GraphifyNode {
    pub fn new(
        id: impl Into<String>,
        numeric_id: u64,
        label: impl Into<String>,
        node_type: GraphifyNodeType,
    ) -> Self {
        Self {
            id: id.into(),
            numeric_id,
            label: label.into(),
            node_type,
            source_file: None,
            source_location: None,
            content_hash: None,
            community_id: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_source(
        mut self,
        file: impl Into<String>,
        location: Option<impl Into<String>>,
    ) -> Self {
        self.source_file = Some(file.into());
        self.source_location = location.map(Into::into);
        self
    }

    pub fn with_hash(mut self, hash: u64) -> Self {
        self.content_hash = Some(hash);
        self
    }

    pub fn with_community(mut self, community_id: u32) -> Self {
        self.community_id = Some(community_id);
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Represents a typed directional dependency edge between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphifyEdge {
    /// Identifier of the source node
    pub source_id: String,
    /// Identifier of the target node
    pub target_id: String,
    /// Semantic relationship classification
    pub relation: GraphifyRelation,
    /// Confidence tier: deterministic AST (Extracted) vs heuristic (Inferred)
    pub confidence: GraphifyConfidence,
    /// Edge weight (default 1.0)
    pub weight: f32,
    /// Location of the reference/call site (e.g. "src/gateway/route.rs:L85")
    pub location: Option<String>,
}

impl GraphifyEdge {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        relation: GraphifyRelation,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            target_id: target_id.into(),
            relation,
            confidence: GraphifyConfidence::Extracted,
            weight: 1.0,
            location: None,
        }
    }

    pub fn with_confidence(mut self, confidence: GraphifyConfidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation_and_attributes() {
        let node = GraphifyNode::new(
            "auth::token",
            101,
            "TokenValidator",
            GraphifyNodeType::Function,
        )
        .with_source("src/auth.rs", Some("L45"))
        .with_hash(0xCAFE_BABE)
        .with_community(3);

        assert_eq!(node.id, "auth::token");
        assert_eq!(node.numeric_id, 101);
        assert_eq!(node.label, "TokenValidator");
        assert_eq!(node.node_type, GraphifyNodeType::Function);
        assert_eq!(node.source_file.as_deref(), Some("src/auth.rs"));
        assert_eq!(node.source_location.as_deref(), Some("L45"));
        assert_eq!(node.content_hash, Some(0xCAFE_BABE));
        assert_eq!(node.community_id, Some(3));
    }

    #[test]
    fn test_edge_creation_and_serialization() {
        let edge = GraphifyEdge::new("auth::token", "db::users", GraphifyRelation::ReadsFrom)
            .with_confidence(GraphifyConfidence::Extracted)
            .with_weight(2.5)
            .with_location("src/auth.rs:L52");

        assert_eq!(edge.source_id, "auth::token");
        assert_eq!(edge.target_id, "db::users");
        assert_eq!(edge.relation, GraphifyRelation::ReadsFrom);
        assert_eq!(edge.weight, 2.5);
        assert!(edge.relation.is_affected_by_default());

        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: GraphifyEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, edge);
    }
}
