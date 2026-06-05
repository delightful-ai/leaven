import pytest

from leaven.evaluation_job import EvaluationItem, EvaluationJob


def test_independent_cases_iterates_independent_job_items() -> None:
    item = EvaluationItem(candidate_id="cand_a", case_id="case_1")
    job = EvaluationJob(
        evaluation_request_id="eval_1",
        kind="independent",
        granularity="per_case",
        evaluator_id="judge",
        items=[item],
    )

    assert list(job.independent_cases()) == [item]


def test_pairwise_cases_requires_pairwise_job_shape() -> None:
    item = EvaluationItem(candidate_ids=["cand_a", "cand_b"], case_id="case_1")
    job = EvaluationJob(
        evaluation_request_id="eval_1",
        kind="pairwise",
        granularity="per_case",
        evaluator_id="judge",
        items=[item],
    )

    assert list(job.pairwise_cases()) == [item]
    with pytest.raises(TypeError, match="independent"):
        list(job.independent_cases())


def test_listwise_cases_rejects_malformed_items() -> None:
    job = EvaluationJob(
        evaluation_request_id="eval_1",
        kind="listwise",
        granularity="per_case",
        evaluator_id="judge",
        items=[EvaluationItem(candidate_id="cand_a", case_id="case_1")],
    )

    with pytest.raises(ValueError, match="candidate_ids"):
        list(job.listwise_cases())
