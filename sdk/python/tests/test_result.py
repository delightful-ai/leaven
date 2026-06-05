import leaven as lv


def test_candidate_summary_includes_score_when_available() -> None:
    candidate = lv.Candidate(
        id="cand_1",
        artifact=lv.PromptArtifact(template="Answer {question}"),
        summary_score=0.875,
    )

    assert candidate.summary() == "cand_1: PromptArtifact score=0.875"


def test_candidate_summary_marks_unscored_candidates() -> None:
    candidate = lv.Candidate(id="cand_2", artifact=lv.SkillBank.empty())

    assert candidate.summary() == "cand_2: SkillBank unscored"
