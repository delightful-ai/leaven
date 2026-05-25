use std::collections::{BTreeMap, BTreeSet};

use leaven_core::Evidence;
use serde::{Deserialize, Serialize};

use crate::OutputRecord;

/// Decision recorded for one patch merge-tree node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentPatchMergeDecision {
    /// Leaf or merge output accepted as usable provenance.
    Accepted {
        /// Human/model rationale for acceptance.
        rationale: String,
    },
    /// Node merged multiple child patches into a consolidated output.
    Merged {
        /// Note about prevalence, support, or consolidation rationale.
        prevalence_note: String,
    },
    /// Node discarded its inputs.
    Discarded {
        /// Reason the inputs were discarded.
        reason: String,
    },
    /// Node output failed parsing or validation.
    ParseFailed {
        /// Parse or validation failure reason.
        reason: String,
    },
}

/// Input for one patch merge-tree node.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeNodeInput {
    /// Stable node id.
    pub node_id: String,
    /// Merge level; leaves are conventionally level 0.
    pub level: u32,
    /// Input patch or child node ids consumed by this node.
    pub input_patch_ids: Vec<String>,
    /// Patch ids accepted or preserved by this node.
    pub accepted_patch_ids: Vec<String>,
    /// Patch ids discarded by this node.
    pub discarded_patch_ids: Vec<String>,
    /// Total support represented by this node.
    pub support_count: u32,
    /// Merge decision for this node.
    pub decision: AgentPatchMergeDecision,
    /// Prompt sent to the merge operator, when this node used one.
    pub prompt: Option<OutputRecord>,
    /// Raw merge response, when one exists.
    pub response: Option<OutputRecord>,
    /// Parse-failure artifact, when parsing failed.
    pub parse_failure: Option<OutputRecord>,
    /// Parsed or blob-backed output patch for this node.
    pub output_patch: Option<OutputRecord>,
}

/// One node in a hierarchical patch merge tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeNode {
    node_id: String,
    level: u32,
    input_patch_ids: Vec<String>,
    accepted_patch_ids: Vec<String>,
    discarded_patch_ids: Vec<String>,
    support_count: u32,
    decision: AgentPatchMergeDecision,
    prompt: Option<OutputRecord>,
    response: Option<OutputRecord>,
    parse_failure: Option<OutputRecord>,
    output_patch: Option<OutputRecord>,
}

impl AgentPatchMergeNode {
    /// Build one merge-tree node.
    pub fn new(input: AgentPatchMergeNodeInput) -> Result<Self, AgentPatchMergeTreeError> {
        if input.node_id.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyNodeId);
        }
        if input.support_count == 0 {
            return Err(AgentPatchMergeTreeError::EmptySupport);
        }
        Ok(Self {
            node_id: input.node_id,
            level: input.level,
            input_patch_ids: input.input_patch_ids,
            accepted_patch_ids: input.accepted_patch_ids,
            discarded_patch_ids: input.discarded_patch_ids,
            support_count: input.support_count,
            decision: input.decision,
            prompt: input.prompt,
            response: input.response,
            parse_failure: input.parse_failure,
            output_patch: input.output_patch,
        })
    }

    /// Stable node id.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Merge level; leaves are conventionally level 0.
    #[must_use]
    pub const fn level(&self) -> u32 {
        self.level
    }

    /// Input patch or child node ids consumed by this node.
    #[must_use]
    pub fn input_patch_ids(&self) -> &[String] {
        &self.input_patch_ids
    }

    /// Patch ids accepted or preserved by this node.
    #[must_use]
    pub fn accepted_patch_ids(&self) -> &[String] {
        &self.accepted_patch_ids
    }

    /// Patch ids discarded by this node.
    #[must_use]
    pub fn discarded_patch_ids(&self) -> &[String] {
        &self.discarded_patch_ids
    }

    /// Total support represented by this node.
    #[must_use]
    pub const fn support_count(&self) -> u32 {
        self.support_count
    }

    /// Merge decision for this node.
    #[must_use]
    pub const fn decision(&self) -> &AgentPatchMergeDecision {
        &self.decision
    }

    /// Prompt sent to the merge operator, when this node used one.
    #[must_use]
    pub const fn prompt(&self) -> Option<&OutputRecord> {
        self.prompt.as_ref()
    }

    /// Raw merge response, when one exists.
    #[must_use]
    pub const fn response(&self) -> Option<&OutputRecord> {
        self.response.as_ref()
    }

    /// Parse-failure artifact, when parsing failed.
    #[must_use]
    pub const fn parse_failure(&self) -> Option<&OutputRecord> {
        self.parse_failure.as_ref()
    }

    /// Parsed or blob-backed output patch for this node.
    #[must_use]
    pub const fn output_patch(&self) -> Option<&OutputRecord> {
        self.output_patch.as_ref()
    }
}

/// Evidence for a hierarchical patch merge tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentPatchMergeTreeEvidence {
    nodes: Vec<AgentPatchMergeNode>,
    final_node_id: String,
    final_diff: Option<OutputRecord>,
}

impl AgentPatchMergeTreeEvidence {
    /// Build merge-tree evidence from validated nodes and a final node id.
    pub fn new(
        nodes: Vec<AgentPatchMergeNode>,
        final_node_id: impl Into<String>,
        final_diff: Option<OutputRecord>,
    ) -> Result<Self, AgentPatchMergeTreeError> {
        if nodes.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyTree);
        }
        let final_node_id = final_node_id.into();
        if final_node_id.is_empty() {
            return Err(AgentPatchMergeTreeError::EmptyFinalNode);
        }
        let mut seen = BTreeSet::new();
        for node in &nodes {
            if !seen.insert(node.node_id.clone()) {
                return Err(AgentPatchMergeTreeError::DuplicateNode {
                    node_id: node.node_id.clone(),
                });
            }
        }
        if !seen.contains(&final_node_id) {
            return Err(AgentPatchMergeTreeError::UnknownFinalNode {
                node_id: final_node_id,
            });
        }
        Ok(Self {
            nodes,
            final_node_id,
            final_diff,
        })
    }

    /// Merge-tree nodes in caller-supplied checkpoint order.
    #[must_use]
    pub fn nodes(&self) -> &[AgentPatchMergeNode] {
        &self.nodes
    }

    /// Final/root node id.
    #[must_use]
    pub fn final_node_id(&self) -> &str {
        &self.final_node_id
    }

    /// Final/root node.
    #[must_use]
    pub fn final_node(&self) -> &AgentPatchMergeNode {
        self.nodes
            .iter()
            .find(|node| node.node_id() == self.final_node_id)
            .expect("constructor verifies final node exists")
    }

    /// Final applied diff or diff artifact, when available.
    #[must_use]
    pub const fn final_diff(&self) -> Option<&OutputRecord> {
        self.final_diff.as_ref()
    }

    /// Sorted merge levels present in this tree.
    #[must_use]
    pub fn levels(&self) -> Vec<u32> {
        self.nodes
            .iter()
            .map(AgentPatchMergeNode::level)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Nodes at one merge level in checkpoint order.
    #[must_use]
    pub fn nodes_at_level(&self, level: u32) -> Vec<&AgentPatchMergeNode> {
        self.nodes
            .iter()
            .filter(|node| node.level() == level)
            .collect()
    }

    /// Unique accepted patch ids in first-seen order.
    #[must_use]
    pub fn accepted_patch_ids(&self) -> Vec<&str> {
        unique_patch_ids(
            self.nodes
                .iter()
                .flat_map(AgentPatchMergeNode::accepted_patch_ids),
        )
    }

    /// Unique discarded patch ids in first-seen order.
    #[must_use]
    pub fn discarded_patch_ids(&self) -> Vec<&str> {
        unique_patch_ids(
            self.nodes
                .iter()
                .flat_map(AgentPatchMergeNode::discarded_patch_ids),
        )
    }

    /// Nodes with parse-failure decisions.
    #[must_use]
    pub fn parse_failed_nodes(&self) -> Vec<&AgentPatchMergeNode> {
        self.nodes
            .iter()
            .filter(|node| matches!(node.decision(), AgentPatchMergeDecision::ParseFailed { .. }))
            .collect()
    }

    /// Sum of support by merge level.
    #[must_use]
    pub fn support_by_level(&self) -> BTreeMap<u32, u32> {
        let mut support = BTreeMap::new();
        for node in &self.nodes {
            *support.entry(node.level()).or_insert(0) += node.support_count();
        }
        support
    }
}

impl Evidence for AgentPatchMergeTreeEvidence {}

fn unique_patch_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> Vec<&'a str> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for id in ids {
        if seen.insert(id.as_str()) {
            unique.push(id.as_str());
        }
    }
    unique
}

/// Merge-tree construction refused invalid input.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AgentPatchMergeTreeError {
    /// Node id was empty.
    #[error("patch merge node id is empty")]
    EmptyNodeId,
    /// Node support count must be positive.
    #[error("patch merge node support count must be positive")]
    EmptySupport,
    /// Tree contained no nodes.
    #[error("patch merge tree is empty")]
    EmptyTree,
    /// Final node id was empty.
    #[error("patch merge tree final node id is empty")]
    EmptyFinalNode,
    /// Node id appeared more than once.
    #[error("patch merge tree has duplicate node id `{node_id}`")]
    DuplicateNode {
        /// Duplicate node id.
        node_id: String,
    },
    /// Final node id was not present in the tree.
    #[error("patch merge tree final node `{node_id}` is not present")]
    UnknownFinalNode {
        /// Missing final node id.
        node_id: String,
    },
}
