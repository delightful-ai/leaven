use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn pinned_dialects_replay_pointer_jsonpath_and_template_deterministically() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let document = package
        .validate_plan_document(&pinned_dialect_plan())
        .unwrap();
    assert_eq!(document.pinned_pointer_count(), 2);
    assert_eq!(document.pinned_jsonpath_count(), 1);
    assert_eq!(document.strict_template_count(), 1);
    let dialects = package.pinned_dialects();
    let document = json!({
        "case": {
            "answer": {
                "score": 7,
                "label": "ok"
            },
            "items": [
                { "name": "alpha", "visible": true },
                { "name": "beta", "visible": false }
            ]
        }
    });

    assert_eq!(
        dialects
            .resolve_json_pointer(&document, "/case/answer/score")
            .unwrap(),
        json!(7)
    );
    assert_eq!(
        dialects
            .extract_json_path(&document, "$.case.items[*].name")
            .unwrap(),
        vec![json!("alpha"), json!("beta")]
    );
    assert_eq!(
        dialects
            .render_template(
                "leaven.mustache.strict.v1",
                "{{case.answer.label}}:{{#case.items}}{{name}};{{/case.items}}",
                &document,
            )
            .unwrap(),
        "ok:alpha;beta;"
    );
}

#[test]
fn pinned_dialects_reject_unpinned_or_executable_syntax() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let dialects = package.pinned_dialects();
    let document = json!({
        "case": {
            "items": [{ "name": "alpha", "visible": true }]
        }
    });

    for pointer in ["case/items/0/name", "/case/~2bad"] {
        assert!(matches!(
            dialects
                .resolve_json_pointer(&document, pointer)
                .unwrap_err(),
            PublicSeamError::InvalidDialect { .. }
        ));
    }

    for path in [
        "$.case.items[?(@.visible)]",
        "$.case.items.length()",
        "$[(@.length-1)]",
        "$.case..name",
        "case.items[0]",
    ] {
        assert!(matches!(
            dialects.extract_json_path(&document, path).unwrap_err(),
            PublicSeamError::InvalidDialect { .. }
        ));
    }

    assert!(matches!(
        dialects
            .render_template("handlebars", "{{case.items.0.name}}", &document)
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));
    for template in [
        "{{> partial}}",
        "{{& case.items.0.name}}",
        "{{case.items.0.name | upper}}",
        "{{{case.items.0.name}}}",
        "{{=<< >>=}}",
    ] {
        assert!(matches!(
            dialects
                .render_template("leaven.mustache.strict.v1", template, &document)
                .unwrap_err(),
            PublicSeamError::InvalidDialect { .. }
        ));
    }

    let mut bad_pointer_plan = pinned_dialect_plan();
    bad_pointer_plan["ops"][0]["expr"]["vars"]["answer"]["input"]["predicate"]["field"] =
        json!("/case/~2bad");
    assert!(matches!(
        package
            .validate_plan_document(&bad_pointer_plan)
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));

    let mut bad_stratified_pointer = pinned_dialect_plan();
    bad_stratified_pointer["ops"][0]["expr"]["vars"]["cases"]["query"]["set"]["by"] =
        json!("/case/~2bad");
    assert!(matches!(
        package
            .validate_plan_document(&bad_stratified_pointer)
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));
    assert!(matches!(
        package
            .validate_plan_document(&bad_graph_filter_pointer_plan("candidate_set"))
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));
    assert!(matches!(
        package
            .validate_plan_document(&bad_graph_filter_pointer_plan("assessment_set"))
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));
    assert!(matches!(
        package
            .validate_plan_document(&schema_valid_precondition_plan("$.case..bad"))
            .unwrap_err(),
        PublicSeamError::InvalidDialect { .. }
    ));
}

#[test]
fn pinned_dialects_do_not_inspect_arbitrary_json_data_slots() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    package
        .validate_plan_document(&extension_payload_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&literal_value_with_prose_field_plan())
        .unwrap();

    let mut predicate_value_with_prose_field = pinned_dialect_plan();
    predicate_value_with_prose_field["ops"][0]["expr"]["vars"]["answer"]["input"]["predicate"]["value"] = json!({
        "field": "user prose, not a JsonPointer"
    });
    package
        .validate_plan_document(&predicate_value_with_prose_field)
        .unwrap();
    let mut predicate_values_with_prose_field = pinned_dialect_plan();
    predicate_values_with_prose_field["ops"][0]["expr"]["vars"]["answer"]["input"]["predicate"] = json!({
        "kind": "in",
        "field": "/case/visible",
        "values": [
            {
                "field": "user prose, not a JsonPointer"
            }
        ]
    });
    let document = package
        .validate_plan_document(&predicate_values_with_prose_field)
        .unwrap();
    assert_eq!(document.pinned_pointer_count(), 2);
    let document = package
        .validate_plan_document(&schema_valid_precondition_plan("$.case.answer"))
        .unwrap();
    assert_eq!(document.pinned_jsonpath_count(), 1);
    package
        .validate_plan_document(&events_filter_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&lm_schema_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&metadata_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&human_review_rubric_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&proposal_causal_with_prose_field_plan())
        .unwrap();
    package
        .validate_plan_document(&assessment_arbitrary_values_with_prose_field_plan())
        .unwrap();
}

fn pinned_dialect_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "dialect001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "message",
                "expr": {
                "kind": "template",
                "dialect": "leaven.mustache.strict.v1",
                "template": "{{answer}}",
                    "vars": {
                        "cases": {
                            "kind": "case_query",
                            "query": {
                                "kind": "resolve_set",
                                "set": {
                                    "kind": "stratified",
                                    "base": {
                                        "kind": "named",
                                        "name": "train"
                                    },
                                    "by": "/case/metadata/topic",
                                    "per_bucket": 2,
                                    "seed": 11
                                },
                                "purpose": "train"
                            }
                        },
                        "answer": {
                            "kind": "extract",
                        "input": {
                            "kind": "filter",
                            "input": {
                                "kind": "literal",
                                "value": {
                                    "case": {
                                        "answer": "ok",
                                        "visible": true
                                    }
                                },
                                "data_classes": ["public"]
                            },
                            "predicate": {
                                "kind": "eq",
                                "field": "/case/visible",
                                "value": true
                            },
                        },
                        "path": "$.case.answer"
                    }
                    }
                }
            }
        ],
        "return": ["message"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn extension_payload_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "dialectext001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "extension_value",
                "expr": {
                    "kind": "extension",
                    "namespace": "x.test",
                    "op": "opaque_payload",
                    "schema_fingerprint": "fp_schema_sha256_extension",
                    "payload": {
                        "field": "user prose, not a JsonPointer",
                        "path": "also user prose"
                    }
                }
            }
        ],
        "return": ["extension_value"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn bad_graph_filter_pointer_plan(source_kind: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": format!("badfilter{source_kind}"),
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "graph_rows",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": source_kind,
                        "filter": {
                            "predicate": {
                                "kind": "exists",
                                "field": "/case/~2bad"
                            }
                        }
                    },
                    "projection": {
                        "kind": "ids"
                    }
                }
            }
        ],
        "return": ["graph_rows"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn events_filter_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "eventfilter001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events",
                        "filter": {
                            "field": "event backend prose, not a JsonPointer"
                        }
                    },
                    "projection": {
                        "kind": "ids"
                    }
                }
            }
        ],
        "return": ["events"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn schema_valid_precondition_plan(path: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "precond001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "event",
                "idempotency_key": "precond-write-0001",
                "preconditions": [
                    {
                        "kind": "schema_valid",
                        "schema_fingerprint": "fp_schema_sha256_precond",
                        "value": {
                            "kind": "extract",
                            "input": {
                                "kind": "literal",
                                "value": {
                                    "case": {
                                        "answer": "ok"
                                    }
                                }
                            },
                            "path": path
                        }
                    }
                ],
                "write": {
                    "kind": "emit_run_event",
                    "event_kind": "plan.dialect.precondition",
                    "payload_schema": "fp_schema_sha256_event",
                    "payload": {
                        "field": "event payload prose"
                    },
                    "visibility": "public"
                }
            }
        ],
        "return": ["event"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn lm_schema_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "lmschema001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "call",
                "name": "completion",
                "idempotency_key": "lm-schema-0001",
                "call": {
                    "kind": "lm_complete",
                    "purpose": "test.dialect",
                    "messages": [
                        {
                            "role": "user",
                            "content": [
                                {
                                    "kind": "text",
                                    "text": "Say ok"
                                }
                            ]
                        }
                    ],
                    "tools": [
                        {
                            "name": "lookup",
                            "input_schema": {
                                "type": "object",
                                "field": "JSON Schema prose, not a JsonPointer"
                            }
                        }
                    ],
                    "output": {
                        "kind": "json_schema",
                        "schema_fingerprint": "fp_schema_sha256_lmout",
                        "schema": {
                            "type": "object",
                            "field": "JSON Schema prose, not a JsonPointer"
                        }
                    },
                    "input_classes": ["public"]
                }
            }
        ],
        "return": ["completion"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn metadata_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "metadata001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "metadata": {
            "field": "metadata prose, not a JsonPointer"
        },
        "ops": [
            {
                "kind": "let",
                "name": "literal_value",
                "expr": {
                    "kind": "literal",
                    "value": "ok",
                    "data_classes": ["public"]
                }
            }
        ],
        "return": ["literal_value"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn literal_value_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "dialectliteral001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "literal_value",
                "expr": {
                    "kind": "literal",
                    "value": {
                        "field": "user prose, not a JsonPointer",
                        "path": "also user prose"
                    },
                    "data_classes": ["public"]
                }
            }
        ],
        "return": ["literal_value"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn human_review_rubric_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "humanrubric001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "call",
                "name": "review",
                "idempotency_key": "human-rubric-0001",
                "call": {
                    "kind": "human_review",
                    "queue": "operators",
                    "prompt": "Review the answer",
                    "rubric": {
                        "field": "rubric prose, not a JsonPointer"
                    },
                    "input_classes": ["public"]
                }
            }
        ],
        "return": ["review"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn proposal_causal_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "proposalcausal001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "proposal_batch",
                "idempotency_key": "proposal-causal-0001",
                "write": {
                    "kind": "submit_proposal_batch",
                    "semantics": "alternatives",
                    "proposals": [
                        {
                            "effect": {
                                "kind": "create",
                                "artifact_type": "answer",
                                "artifact_schema": "fp_schema_sha256_answer",
                                "artifact": {
                                    "kind": "literal",
                                    "value": "ok"
                                }
                            },
                            "causal": {
                                "field": "causal prose, not a JsonPointer"
                            },
                            "informed_by": {
                                "kind": "literal",
                                "value": "source"
                            }
                        }
                    ]
                }
            }
        ],
        "return": ["proposal_batch"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn assessment_arbitrary_values_with_prose_field_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "assessmentvalues001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "assessments",
                "idempotency_key": "assessment-values-0001",
                "write": {
                    "kind": "submit_assessments",
                    "evaluation_request_id": "evalreq_dialect",
                    "assessments": [
                        {
                            "kind": "independent",
                            "candidate": "cand_dialect",
                            "target": {
                                "field": "target prose, not a JsonPointer"
                            },
                            "score": {
                                "value": 1.0,
                                "output": {
                                    "kind": "structured",
                                    "summary": "ok",
                                    "value": {
                                        "candidate": "cand_dialect",
                                        "output": "ok"
                                    },
                                    "visibility": "public",
                                    "data_classes": ["candidate.output", "public"]
                                }
                            },
                            "preference": {
                                "field": "preference prose, not a JsonPointer"
                            },
                            "ranking": {
                                "field": "ranking prose, not a JsonPointer"
                            },
                            "evidence": {
                                "schema_version": "leaven.evidence_envelope.v1",
                                "target_derived": false,
                                "public": {
                                    "summary": "ok",
                                    "data_classes": ["public"]
                                },
                                "redaction_policy": {
                                    "optimizer": "score_only",
                                    "reflector": "score_only",
                                    "operator": "score_only"
                                },
                                "producer": {
                                    "stage_call_id": "sc_dialect"
                                },
                                "source_receipts": {
                                    "read": ["qrec_dialect_assessment"],
                                    "effect": []
                                }
                            },
                            "read_receipts": ["qrec_dialect_assessment"],
                            "replayability": "pure_read"
                        }
                    ]
                }
            }
        ],
        "return": ["assessments"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
