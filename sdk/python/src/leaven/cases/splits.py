"""`lv.cases.splits(...)` — bundle train/val/test case sets."""

from ..case import CaseSet, CaseSplits


def splits(
    *,
    train: CaseSet,
    val: CaseSet | None = None,
    test: CaseSet | None = None,
) -> CaseSplits:
    """Bundle train/val/test case sets into a `CaseSplits`.

    Pass to `lv.optimize(train=..., val=..., test=...)` either as separate
    arguments or via `lv.optimize(splits=splits)` (when splits-shaped).
    """
    return CaseSplits(train=train, val=val, test=test)


__all__ = ["splits"]
