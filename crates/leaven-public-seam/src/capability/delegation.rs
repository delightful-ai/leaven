use std::collections::BTreeSet;

use super::{
    AggregateBudgets, CapabilityConstraintValue, CapabilityDenial, CapabilityDenialKind,
    CapabilityDocument, CapabilityResourceValue, Grant, TokenBinding, parse_timestamp,
};

/// Validated parent-child capability lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDelegation {
    parent_capability_fingerprint: String,
    child_capability_fingerprint: String,
    allowed_actions: Vec<String>,
}

impl CapabilityDelegation {
    /// Parent capability fingerprint recorded by the child.
    pub fn parent_capability_fingerprint(&self) -> &str {
        &self.parent_capability_fingerprint
    }

    /// Child capability fingerprint.
    pub fn child_capability_fingerprint(&self) -> &str {
        &self.child_capability_fingerprint
    }

    /// Actions delegated to the child.
    pub fn allowed_actions(&self) -> &[String] {
        &self.allowed_actions
    }
}

impl CapabilityDocument {
    /// Validates that a child capability only narrows this parent capability.
    pub fn validate_delegation(
        &self,
        child: &Self,
    ) -> Result<CapabilityDelegation, CapabilityDenial> {
        if !self.delegation.may_delegate {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "parent capability is not delegable",
            ));
        }
        if child.parent_capability_fingerprint.as_deref() != Some(self.capability_fingerprint()) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child capability does not record parent lineage",
            ));
        }
        let child_expires_at = parse_timestamp(child.expires_at())
            .map_err(|error| CapabilityDenial::from_invalid_document(&error))?;
        let parent_expires_at = parse_timestamp(self.expires_at())
            .map_err(|error| CapabilityDenial::from_invalid_document(&error))?;
        if child_expires_at > parent_expires_at {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child expiry widens parent expiry",
            ));
        }
        let expiry_narrowed = child_expires_at < parent_expires_at;
        let binding_narrowed =
            ensure_binding_attenuates(&self.token_binding, &child.token_binding)?;
        let budget_narrowed = ensure_budget_attenuates(&self.budgets, &child.budgets)?;
        let grants_narrowed = self.ensure_grants_attenuate(child)?;
        let narrowed = expiry_narrowed || binding_narrowed || budget_narrowed || grants_narrowed;
        ensure_delegation_policy_attenuates(self, child)?;
        if self.delegation.must_attenuate && !narrowed {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child does not narrow parent operational authority",
            ));
        }

        Ok(CapabilityDelegation {
            parent_capability_fingerprint: self.capability_fingerprint.clone(),
            child_capability_fingerprint: child.capability_fingerprint.clone(),
            allowed_actions: child
                .grants
                .iter()
                .map(|grant| grant.action.clone())
                .collect(),
        })
    }

    fn ensure_grants_attenuate(&self, child: &Self) -> Result<bool, CapabilityDenial> {
        let allowed_actions = self
            .delegation
            .allowed_actions
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parent_actions = self.grant_actions().collect::<BTreeSet<_>>();
        let child_actions = child.grant_actions().collect::<BTreeSet<_>>();
        if !child_actions.is_subset(&parent_actions) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                "child grants actions absent from parent",
            ));
        }
        let mut narrowed = child_actions.len() < parent_actions.len();
        for child_grant in &child.grants {
            if !allowed_actions.contains(&child_grant.action) {
                return Err(CapabilityDenial::new(
                    CapabilityDenialKind::Delegation,
                    format!("child action `{}` is not delegable", child_grant.action),
                ));
            }
            let Some(parent_grant) = self.grant(&child_grant.action) else {
                return Err(CapabilityDenial::new(
                    CapabilityDenialKind::Delegation,
                    format!(
                        "child action `{}` is not granted by parent",
                        child_grant.action
                    ),
                ));
            };
            narrowed |= ensure_resource_attenuates(parent_grant, child_grant)?;
            narrowed |= ensure_constraints_attenuate(parent_grant, child_grant)?;
            narrowed |= ensure_grant_limits_attenuate(parent_grant, child_grant)?;
        }
        Ok(narrowed)
    }
}

impl TokenBinding {
    fn kind(&self) -> &'static str {
        match self {
            Self::OpaqueLookup { .. } => "opaque_lookup",
            Self::SignedJwt { .. } => "signed_jwt",
            Self::MtlsBound { .. } => "mtls_bound",
        }
    }
}

fn ensure_binding_attenuates(
    parent: &TokenBinding,
    child: &TokenBinding,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (
            TokenBinding::OpaqueLookup {
                lookup_audience: parent_audience,
                ..
            },
            TokenBinding::OpaqueLookup {
                lookup_audience: child_audience,
                ..
            },
        ) => ensure_optional_string_binding_attenuates(
            "lookup_audience",
            parent_audience.as_deref(),
            child_audience.as_deref(),
        ),
        (
            TokenBinding::SignedJwt {
                alg: parent_alg,
                kid: parent_kid,
            },
            TokenBinding::SignedJwt {
                alg: child_alg,
                kid: child_kid,
            },
        ) if parent_alg == child_alg && parent_kid == child_kid => Ok(false),
        (
            TokenBinding::MtlsBound {
                certificate_fingerprint: parent_fingerprint,
            },
            TokenBinding::MtlsBound {
                certificate_fingerprint: child_fingerprint,
            },
        ) if parent_fingerprint == child_fingerprint => Ok(false),
        _ if parent.kind() == child.kind() => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child token binding weakens parent binding authority",
        )),
        _ => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child token binding widens parent binding mode",
        )),
    }
}

fn ensure_optional_string_binding_attenuates(
    key: &str,
    parent: Option<&str>,
    child: Option<&str>,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (Some(parent), Some(child)) if parent == child => Ok(false),
        (Some(_), _) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child token binding omits or changes parent `{key}`"),
        )),
        (None, Some(_)) => Ok(true),
        (None, None) => Ok(false),
    }
}

fn ensure_budget_attenuates(
    parent: &AggregateBudgets,
    child: &AggregateBudgets,
) -> Result<bool, CapabilityDenial> {
    let mut narrowed = false;
    for (name, parent, child) in [
        (
            "max_total_usd_micro",
            parent.total_usd_micro,
            child.total_usd_micro,
        ),
        ("max_lm_usd_micro", parent.lm_usd_micro, child.lm_usd_micro),
        (
            "max_agent_usd_micro",
            parent.agent_usd_micro,
            child.agent_usd_micro,
        ),
        (
            "max_evaluator_usd_micro",
            parent.evaluator_usd_micro,
            child.evaluator_usd_micro,
        ),
        ("max_wall_ms", parent.wall_ms, child.wall_ms),
        (
            "max_concurrent_calls",
            parent.concurrent_calls,
            child.concurrent_calls,
        ),
        ("max_plan_nodes", parent.plan_nodes, child.plan_nodes),
        (
            "max_materialized_bytes",
            parent.materialized_bytes,
            child.materialized_bytes,
        ),
    ] {
        narrowed |= ensure_optional_u64_attenuates(name, parent, child)?;
    }
    Ok(narrowed)
}

fn ensure_resource_attenuates(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    for key in parent.resource.keys() {
        if !child.resource.contains_key(key) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child omits parent resource `{key}`"),
            ));
        }
    }
    let mut narrowed = false;
    for (key, child_value) in child.resource.entries() {
        let Some(parent_value) = parent.resource.get(key) else {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child resource `{key}` is not present in parent"),
            ));
        };
        if !parent_value.allows(child_value) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child resource `{key}` widens parent"),
            ));
        }
        narrowed |= resource_value_narrows(parent_value, child_value);
    }
    Ok(narrowed)
}

fn ensure_constraints_attenuate(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    let keys = parent
        .constraints
        .keys()
        .into_iter()
        .chain(child.constraints.keys())
        .collect::<BTreeSet<_>>();
    let mut narrowed = false;
    for key in keys {
        let parent_value = parent.constraints.get(key);
        let child_value = child.constraints.get(key);
        narrowed |= if key.starts_with("forbidden_") {
            ensure_forbidden_constraint_attenuates(key, parent_value, child_value)?
        } else {
            ensure_allowed_constraint_attenuates(key, parent_value, child_value)?
        };
    }
    Ok(narrowed)
}

fn ensure_allowed_constraint_attenuates(
    key: &str,
    parent: Option<CapabilityConstraintValue>,
    child: Option<CapabilityConstraintValue>,
) -> Result<bool, CapabilityDenial> {
    match (&parent, &child) {
        (None, None) => Ok(false),
        (Some(_), None) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child omits parent constraint `{key}`"),
        )),
        (None, Some(_)) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child constraint `{key}` is absent from parent"),
        )),
        (Some(parent), Some(child)) if parent.allows(child) => Ok(parent.narrows(child)),
        (Some(_), Some(_)) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child constraint `{key}` widens parent"),
        )),
    }
}

fn ensure_forbidden_constraint_attenuates(
    key: &str,
    parent: Option<CapabilityConstraintValue>,
    child: Option<CapabilityConstraintValue>,
) -> Result<bool, CapabilityDenial> {
    match (&parent, &child) {
        (None, None) => Ok(false),
        (Some(_), None) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child omits parent forbidden constraint `{key}`"),
        )),
        (None, Some(_)) => Ok(true),
        (Some(parent), Some(child)) if parent.forbidden_attenuates(child) => {
            Ok(parent.forbidden_narrows(child))
        }
        (Some(_), Some(_)) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child forbidden constraint `{key}` weakens parent"),
        )),
    }
}

fn ensure_grant_limits_attenuate(parent: &Grant, child: &Grant) -> Result<bool, CapabilityDenial> {
    let mut narrowed = false;
    for key in [
        "max_usd_micro",
        "max_calls",
        "max_concurrent",
        "timeout_s",
        "max_rows",
        "max_materialized_bytes",
    ] {
        narrowed |=
            ensure_optional_u64_attenuates(key, parent.limit_value(key), child.limit_value(key))?;
    }
    Ok(narrowed)
}

fn ensure_delegation_policy_attenuates(
    parent: &CapabilityDocument,
    child: &CapabilityDocument,
) -> Result<(), CapabilityDenial> {
    if parent.delegation.max_depth == 0 {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "parent delegation depth is exhausted",
        ));
    }
    if parent.delegation.must_attenuate && !child.delegation.must_attenuate {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child disables parent attenuation requirement",
        ));
    }
    if !parent.delegation.may_delegate && child.delegation.may_delegate {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child enables delegation denied by parent",
        ));
    }
    if child.delegation.may_delegate && child.delegation.max_depth == 0 {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "delegable child must carry remaining delegation depth",
        ));
    }
    if child.delegation.max_depth >= parent.delegation.max_depth {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "child delegation depth does not attenuate parent",
        ));
    }
    if !child.delegation.may_delegate && !child.delegation.allowed_actions.is_empty() {
        return Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            "non-delegable child cannot carry delegable actions",
        ));
    }

    let child_grant_actions = child.grant_actions().collect::<BTreeSet<_>>();
    let parent_delegable_actions = parent
        .delegation
        .allowed_actions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for action in &child.delegation.allowed_actions {
        if !parent_delegable_actions.contains(action) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child delegation action `{action}` is not allowed by parent"),
            ));
        }
        if !child_grant_actions.contains(action.as_str()) {
            return Err(CapabilityDenial::new(
                CapabilityDenialKind::Delegation,
                format!("child delegation action `{action}` is not granted to child"),
            ));
        }
    }

    Ok(())
}

fn ensure_optional_u64_attenuates(
    key: &str,
    parent: Option<u64>,
    child: Option<u64>,
) -> Result<bool, CapabilityDenial> {
    match (parent, child) {
        (Some(_), None) => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child omits parent limit `{key}`"),
        )),
        (Some(parent), Some(child)) if child > parent => Err(CapabilityDenial::new(
            CapabilityDenialKind::Delegation,
            format!("child limit `{key}` widens parent"),
        )),
        (Some(parent), Some(child)) => Ok(child < parent),
        (None, Some(_)) => Ok(true),
        (None, None) => Ok(false),
    }
}

fn resource_value_narrows(
    parent: &CapabilityResourceValue,
    child: &CapabilityResourceValue,
) -> bool {
    parent.allows(child) && !child.allows(parent)
}
