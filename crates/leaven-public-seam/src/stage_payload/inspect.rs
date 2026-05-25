fn inspect_reflect_request(
    object: &serde_json::Map<String, Value>,
) -> Result<usize, PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "query_policy_fingerprint")?;
    if required_string(object.get("target_safety"), "target_safety")? != "target_safe_projection" {
        return Err(invalid_stage_payload(
            "reflector request must declare target_safe_projection",
        ));
    }
    require_parent_source_ref(object)?;
    require_reflect_surface_context(object)?;
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    reject_target_leakage(object.get("source_refs"), "reflector request source_refs")?;
    let top_level_source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    let examples = required_array(object.get("examples"), "examples")?;
    if examples.is_empty() {
        return Err(invalid_stage_payload(
            "reflector request must carry target-safe examples",
        ));
    }
    for example in examples {
        let example = example
            .as_object()
            .ok_or_else(|| invalid_stage_payload("reflective examples must be objects"))?;
        require_non_empty_array(example.get("source_refs"), "examples.source_refs")?;
        reject_target_leakage(example.get("source_refs"), "reflector example source_refs")?;
        require_source_ref_coverage(
            &top_level_source_refs,
            example.get("source_refs"),
            "reflector request source_refs",
            "reflective example source ref",
        )?;
        reject_target_leakage(example.get("input"), "reflector example input")?;
        reject_target_leakage(example.get("output"), "reflector example output")?;
        reject_target_leakage(example.get("feedback"), "reflector example feedback")?;
        reject_target_leakage(example.get("side_info"), "reflector example side_info")?;
        reject_target_leakage(example.get("score"), "reflector example score")?;
        let example_data_classes =
            string_array(example.get("data_classes"), "examples.data_classes")?;
        if example_data_classes.is_empty() {
            return Err(invalid_stage_payload(
                "reflector examples must carry data classes",
            ));
        }
        require_output_record_data_class_coverage(
            &example_data_classes,
            example
                .get("score")
                .and_then(|score| score.as_object())
                .and_then(|score| score.get("output")),
            "reflector example data_classes",
            "score output data class",
        )?;
        require_assessed_output_data_class(
            example
                .get("score")
                .and_then(|score| score.as_object())
                .and_then(|score| score.get("output")),
            "reflector example score output",
        )?;
        for data_class in example_data_classes {
            if contains_case_target_marker(&data_class) {
                return Err(invalid_stage_payload(
                    "reflector examples must not carry case.target data classes",
                ));
            }
        }
    }
    Ok(examples.len())
}

fn inspect_reflection_result(
    object: &serde_json::Map<String, Value>,
    source_ref_count: usize,
    read_receipt_count: usize,
    data_classes: &[String],
) -> Result<(), PublicSeamError> {
    if required_string(object.get("summary"), "summary")?
        .trim()
        .is_empty()
    {
        return Err(invalid_stage_payload(
            "reflection_result summary must be non-empty",
        ));
    }
    if source_ref_count == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry source refs",
        ));
    }
    if read_receipt_count == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry read receipts",
        ));
    }
    require_read_receipt_refs(object.get("read_receipts"), "read_receipts")?;
    if data_classes.is_empty() {
        return Err(invalid_stage_payload(
            "reflection_result must carry data classes",
        ));
    }
    validate_reflection_diagnosis_sources(object)?;
    Ok(())
}

fn inspect_propose_request(
    object: &serde_json::Map<String, Value>,
) -> Result<(Vec<StageProposalEffect>, usize), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "query_policy_fingerprint")?;
    require_field(object, "surface_fingerprint")?;
    require_parent_source_ref(object)?;
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    let reflection_value = object
        .get("reflection_result")
        .ok_or_else(|| invalid_stage_payload("propose request must carry ReflectionResult"))?;
    let reflection = StagePayloadDocument::from_schema_valid_value(reflection_value)?;
    if reflection.role() != StagePayloadRole::ReflectionResult {
        return Err(invalid_stage_payload(
            "propose request must consume a ReflectionResult payload",
        ));
    }
    let reflection_source_refs = source_ref_set(
        reflection_value
            .as_object()
            .and_then(|object| object.get("source_refs")),
        "reflection_result.source_refs",
    )?;
    require_reflection_source_refs(object, reflection_source_refs)?;
    let effects = required_array(object.get("allowed_effects"), "allowed_effects")?
        .iter()
        .map(|effect| {
            required_string(Some(effect), "allowed_effects").and_then(StageProposalEffect::parse)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if effects.is_empty() {
        return Err(invalid_stage_payload(
            "propose request must declare allowed effects",
        ));
    }
    let change_schema_count = array_len(
        object.get("allowed_change_schemas"),
        "allowed_change_schemas",
    )?;
    if effects.iter().any(|effect| effect.requires_change_schema()) && change_schema_count == 0 {
        return Err(invalid_stage_payload(
            "change proposal effects must declare allowed_change_schemas",
        ));
    }
    Ok((effects, change_schema_count))
}

fn validate_submit_proposal_batch_for_handoff(
    write: &serde_json::Map<String, Value>,
    allowed_effects: &BTreeSet<StageProposalEffect>,
    allowed_change_schemas: &BTreeSet<String>,
    reflection_read_receipts: &[String],
    document: &mut ReflectProposeSubmissionDocument,
) -> Result<(), PublicSeamError> {
    let proposals = write
        .get("proposals")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_stage_payload("submit_proposal_batch must carry proposals"))?;
    for proposal in proposals {
        let proposal = proposal
            .as_object()
            .ok_or_else(|| invalid_stage_payload("proposal submission entries must be objects"))?;
        let effect = proposal
            .get("effect")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_stage_payload("proposal submission must carry effect"))?;
        let effect_kind =
            StageProposalEffect::parse(required_string(effect.get("kind"), "effect.kind")?)?;
        if !allowed_effects.contains(&effect_kind) {
            return Err(invalid_stage_payload(format!(
                "proposal effect `{}` is outside ProposeRequest.allowed_effects",
                effect_kind.as_str()
            )));
        }
        validate_proposal_stage_provenance(proposal, document.handoff.propose_stage_receipt())?;
        validate_proposal_reflection_read_receipts(proposal, reflection_read_receipts)?;
        validate_proposal_causal_parent(proposal, document.handoff.parent())?;
        document.stage_provenance_links += 1;
        document.proposal_count += 1;
        match effect_kind {
            StageProposalEffect::Create => document.create_effects += 1,
            StageProposalEffect::Change => {
                document.change_effects += 1;
                validate_change_effect_for_handoff(effect, allowed_change_schemas, document)?;
            }
            StageProposalEffect::ChangeFromWorkspaceDiff => {
                document.workspace_diff_effects += 1;
                validate_change_effect_for_handoff(effect, allowed_change_schemas, document)?;
            }
            StageProposalEffect::ChangeFromAgentSession => {
                document.agent_session_effects += 1;
                validate_agent_session_proposal_receipt(proposal, effect)?;
                validate_change_effect_for_handoff(effect, allowed_change_schemas, document)?;
            }
        }
    }
    Ok(())
}

fn validate_change_effect_for_handoff(
    effect: &serde_json::Map<String, Value>,
    allowed_change_schemas: &BTreeSet<String>,
    document: &ReflectProposeSubmissionDocument,
) -> Result<(), PublicSeamError> {
    let target = effect
        .get("target")
        .ok_or_else(|| invalid_stage_payload("change proposal effect must carry target"))?;
    let target = source_ref_key(target)?;
    if target != document.handoff.parent() {
        return Err(invalid_stage_payload(
            "change proposal target must match the reflected parent candidate",
        ));
    }
    if required_string(
        effect.get("surface_fingerprint"),
        "effect.surface_fingerprint",
    )? != document.handoff.surface_fingerprint()
    {
        return Err(invalid_stage_payload(
            "change proposal surface must match the ProposeRequest surface fingerprint",
        ));
    }
    let change_schema = required_string(effect.get("change_schema"), "effect.change_schema")?;
    if !allowed_change_schemas.contains(change_schema) {
        return Err(invalid_stage_payload(format!(
            "proposal change_schema `{change_schema}` is outside ProposeRequest.allowed_change_schemas"
        )));
    }
    Ok(())
}

fn validate_proposal_stage_provenance(
    proposal: &serde_json::Map<String, Value>,
    propose_stage_receipt: &str,
) -> Result<(), PublicSeamError> {
    let informed_by = proposal.get("informed_by").ok_or_else(|| {
        invalid_stage_payload("proposal submission must carry informed_by stage provenance")
    })?;
    if literal_expr_array_contains_string(informed_by, propose_stage_receipt) {
        Ok(())
    } else {
        Err(invalid_stage_payload(
            "proposal submission informed_by must cite the proposer stage receipt",
        ))
    }
}

fn validate_proposal_reflection_read_receipts(
    proposal: &serde_json::Map<String, Value>,
    reflection_read_receipts: &[String],
) -> Result<(), PublicSeamError> {
    let proposal_read_receipts = receipt_ref_ids(proposal.get("read_receipts"), "read_receipts")?;
    for receipt in reflection_read_receipts {
        if !proposal_read_receipts.contains(receipt) {
            return Err(invalid_stage_payload(format!(
                "proposal read_receipts must preserve reflection read receipt `{receipt}`"
            )));
        }
    }
    Ok(())
}

fn validate_proposal_causal_parent(
    proposal: &serde_json::Map<String, Value>,
    parent: &str,
) -> Result<(), PublicSeamError> {
    let causal_inputs = proposal
        .get("causal")
        .and_then(|causal| causal.as_object())
        .and_then(|causal| causal.get("inputs"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_stage_payload("proposal causal.inputs must be an array"))?;
    for input in causal_inputs {
        if source_ref_key(input)? == parent {
            return Ok(());
        }
    }
    Err(invalid_stage_payload(
        "proposal causal.inputs must include the reflected parent candidate",
    ))
}

fn validate_agent_session_proposal_receipt(
    proposal: &serde_json::Map<String, Value>,
    effect: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let agent_receipt = receipt_ref_id(
        effect.get("agent_receipt").ok_or_else(|| {
            invalid_stage_payload("agent-session proposal must carry agent_receipt")
        })?,
        "effect.agent_receipt",
    )?;
    let read_receipts = receipt_ref_ids(proposal.get("read_receipts"), "read_receipts")?;
    if read_receipts.contains(&agent_receipt) {
        Ok(())
    } else {
        Err(invalid_stage_payload(format!(
            "agent-session proposal read_receipts must include agent receipt `{agent_receipt}`"
        )))
    }
}

fn require_parent_source_ref(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let parent = object
        .get("parent")
        .ok_or_else(|| invalid_stage_payload("stage payload must carry `parent`"))?;
    let parent_ref = source_ref_key(parent)?;
    let source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    if !source_refs.contains(&parent_ref) {
        return Err(invalid_stage_payload(
            "stage payload source_refs must include the parent candidate",
        ));
    }
    Ok(())
}

fn require_reflect_surface_context(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    require_field(object, "surface_fingerprint")?;
    if object.get("part").is_none() && object.get("part_label").is_none() {
        return Err(invalid_stage_payload(
            "reflector request must carry part or part_label context",
        ));
    }
    Ok(())
}

fn validate_reflection_diagnosis_sources(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let failure_modes = array_len(object.get("failure_modes"), "failure_modes")?;
    let surface_suggestions = array_len(object.get("surface_suggestions"), "surface_suggestions")?;
    if failure_modes + surface_suggestions == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry source-backed diagnosis",
        ));
    }
    let top_level_source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    require_nested_source_refs(
        &top_level_source_refs,
        object.get("failure_modes"),
        "failure_modes",
    )?;
    require_nested_source_refs(
        &top_level_source_refs,
        object.get("surface_suggestions"),
        "surface_suggestions",
    )?;
    Ok(())
}

fn require_nested_source_refs(
    top_level_source_refs: &BTreeSet<String>,
    value: Option<&Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    let Some(value) = value else {
        return Ok(());
    };
    let items = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    for item in items {
        let item = item.as_object().ok_or_else(|| {
            invalid_stage_payload(format!(
                "stage payload field `{field}` entries must be objects"
            ))
        })?;
        require_non_empty_array(item.get("source_refs"), &format!("{field}.source_refs"))?;
        require_source_ref_coverage(
            top_level_source_refs,
            item.get("source_refs"),
            "reflection_result source_refs",
            &format!("{field} source ref"),
        )?;
    }
    Ok(())
}

fn require_source_ref_coverage(
    top_level_source_refs: &BTreeSet<String>,
    nested_source_refs: Option<&Value>,
    top_level_field: &str,
    nested_label: &str,
) -> Result<(), PublicSeamError> {
    for source_ref in source_ref_set(nested_source_refs, nested_label)? {
        if !top_level_source_refs.contains(&source_ref) {
            return Err(invalid_stage_payload(format!(
                "{top_level_field} must cover {nested_label} `{source_ref}`"
            )));
        }
    }
    Ok(())
}

fn require_reflection_source_refs(
    propose: &serde_json::Map<String, Value>,
    reflection_source_refs: BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let propose_source_refs = source_ref_set(propose.get("source_refs"), "source_refs")?;
    for source_ref in reflection_source_refs {
        if !propose_source_refs.contains(&source_ref) {
            return Err(invalid_stage_payload(format!(
                "propose request source_refs must preserve reflection source ref `{source_ref}`"
            )));
        }
    }
    Ok(())
}

fn require_output_record_data_class_coverage(
    carrier: &[String],
    value: Option<&Value>,
    top_level_field: &str,
    nested_label: &str,
) -> Result<(), PublicSeamError> {
    for data_class in collect_output_record_data_classes(value, nested_label)? {
        if !carrier.contains(&data_class) {
            return Err(invalid_stage_payload(format!(
                "{top_level_field} must cover {nested_label} `{data_class}`"
            )));
        }
    }
    Ok(())
}

fn require_assessed_output_data_class(
    value: Option<&Value>,
    context: &str,
) -> Result<(), PublicSeamError> {
    let output = value
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_stage_payload(format!("{context} must be an object")))?;
    let data_classes = output
        .get("data_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_stage_payload(format!("{context} must carry data_classes")))?;
    let carries_assessed_output = data_classes.iter().any(|class| {
        matches!(
            class.as_str(),
            Some("candidate.output" | "candidate.artifact")
        )
    });
    if !carries_assessed_output {
        return Err(invalid_stage_payload(format!(
            "{context} must carry candidate.output or candidate.artifact data class"
        )));
    }
    Ok(())
}

fn collect_output_record_data_classes(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let mut data_classes = BTreeSet::new();
    let Some(value) = value else {
        return Ok(data_classes);
    };
    let output = value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} must be an object")))?;
    if let Some(classes) = output.get("data_classes") {
        data_classes.extend(string_array(Some(classes), field)?);
    }
    if let Some(blob_ref) = output.get("blob_ref") {
        let blob_ref = blob_ref
            .as_object()
            .ok_or_else(|| invalid_stage_payload(format!("{field} blob_ref must be an object")))?;
        if let Some(classes) = blob_ref.get("data_classes") {
            data_classes.extend(string_array(Some(classes), field)?);
        }
    }
    if let Some(trace_refs) = output.get("trace_refs") {
        let trace_refs = trace_refs
            .as_array()
            .ok_or_else(|| invalid_stage_payload(format!("{field} trace_refs must be an array")))?;
        for trace in trace_refs {
            let trace = trace.as_object().ok_or_else(|| {
                invalid_stage_payload(format!("{field} trace_refs entries must be objects"))
            })?;
            if let Some(classes) = trace.get("data_classes") {
                data_classes.extend(string_array(Some(classes), field)?);
            }
        }
    }
    Ok(data_classes)
}

fn inspect_runner_request(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    if object.get("target_forbidden") != Some(&Value::Bool(true)) {
        return Err(invalid_stage_payload(
            "runner request must declare target_forbidden=true",
        ));
    }
    reject_target_leakage(object.get("case_input"), "runner case_input")?;
    Ok(())
}

fn inspect_score_context(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "output")?;
    require_assessed_output_data_class(object.get("output"), "score context output")?;
    if let Some(target_handle) = object.get("target_handle") {
        let target_handle = required_string(Some(target_handle), "target_handle")?;
        let case = required_string(object.get("case"), "case")?;
        if target_handle != case {
            return Err(invalid_stage_payload(
                "score context target_handle must bind to the scored case",
            ));
        }
    }
    let output_classes = string_array(
        object
            .get("output")
            .and_then(|output| output.get("data_classes")),
        "output.data_classes",
    )?;
    require_output_record_data_class_coverage(
        &output_classes,
        object.get("output"),
        "score context output data_classes",
        "score output nested data class",
    )?;
    Ok(())
}

fn inspect_judge_context(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    if array_len(object.get("outputs"), "outputs")? == 0 {
        return Err(invalid_stage_payload(
            "judge context must carry assessed outputs",
        ));
    }
    for output in required_array(object.get("outputs"), "outputs")? {
        require_assessed_output_data_class(Some(output), "judge context output")?;
        let output_classes = string_array(output.get("data_classes"), "outputs.data_classes")?;
        require_output_record_data_class_coverage(
            &output_classes,
            Some(output),
            "judge context output data_classes",
            "judge output nested data class",
        )?;
    }
    Ok(())
}

fn inspect_schema_bound_payload(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "payload_schema")?;
    Ok(())
}
