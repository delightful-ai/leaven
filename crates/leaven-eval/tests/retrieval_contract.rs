use leaven_eval::{
    RankedRetrievalEvaluation, RankedRetrievalEvaluationError, RetrievalItemId, RetrievalQuery,
    RetrievalRanking,
};

#[test]
fn ranked_retrieval_evaluation_reports_recall_at_k() {
    let evaluation = RankedRetrievalEvaluation::evaluate(
        [
            RetrievalItemId::from("alpha"),
            RetrievalItemId::from("beta"),
            RetrievalItemId::from("gamma"),
        ],
        vec![
            RetrievalQuery::new("q-alpha", [RetrievalItemId::from("alpha")]).unwrap(),
            RetrievalQuery::new("q-beta", [RetrievalItemId::from("beta")]).unwrap(),
            RetrievalQuery::new("q-gamma", [RetrievalItemId::from("gamma")]).unwrap(),
        ],
        vec![
            RetrievalRanking::new(
                "q-alpha",
                [
                    RetrievalItemId::from("beta"),
                    RetrievalItemId::from("alpha"),
                ],
            )
            .unwrap(),
            RetrievalRanking::new(
                "q-beta",
                [
                    RetrievalItemId::from("gamma"),
                    RetrievalItemId::from("beta"),
                ],
            )
            .unwrap(),
            RetrievalRanking::new("q-gamma", [RetrievalItemId::from("gamma")]).unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(evaluation.query_count(), 3);
    assert_eq!(evaluation.hit_count_at(1), 1);
    assert_eq!(evaluation.hit_count_at(2), 3);
    assert!((evaluation.recall_at(1) - (1.0 / 3.0)).abs() < f64::EPSILON);
    assert!((evaluation.recall_at(2) - 1.0).abs() < f64::EPSILON);
}

#[test]
fn ranked_retrieval_evaluation_refuses_ambiguous_inputs() {
    let empty_relevance = RetrievalQuery::new("q-empty", []).unwrap_err();
    assert_eq!(
        empty_relevance,
        RankedRetrievalEvaluationError::EmptyRelevantItems {
            query_id: "q-empty".to_owned(),
        }
    );

    let duplicate_ranking = RetrievalRanking::new(
        "q-alpha",
        [
            RetrievalItemId::from("alpha"),
            RetrievalItemId::from("alpha"),
        ],
    )
    .unwrap_err();
    assert_eq!(
        duplicate_ranking,
        RankedRetrievalEvaluationError::DuplicateRankedItem {
            query_id: "q-alpha".to_owned(),
            item: RetrievalItemId::from("alpha"),
        }
    );

    let unknown_relevant = RankedRetrievalEvaluation::evaluate(
        [RetrievalItemId::from("alpha")],
        vec![RetrievalQuery::new("q-missing", [RetrievalItemId::from("missing")]).unwrap()],
        vec![RetrievalRanking::new("q-missing", [RetrievalItemId::from("alpha")]).unwrap()],
    )
    .unwrap_err();
    assert_eq!(
        unknown_relevant,
        RankedRetrievalEvaluationError::UnknownRelevantItem {
            query_id: "q-missing".to_owned(),
            item: RetrievalItemId::from("missing"),
        }
    );

    let missing_ranking = RankedRetrievalEvaluation::evaluate(
        [RetrievalItemId::from("alpha")],
        vec![RetrievalQuery::new("q-alpha", [RetrievalItemId::from("alpha")]).unwrap()],
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(
        missing_ranking,
        RankedRetrievalEvaluationError::MissingRanking {
            query_id: "q-alpha".to_owned(),
        }
    );
}
