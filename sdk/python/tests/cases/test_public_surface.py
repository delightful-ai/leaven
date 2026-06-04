"""Tests for the public `leaven.cases` namespace."""

import leaven as lv


def test_cases_namespace_excludes_unimplemented_parquet_loader() -> None:
    """Regression: the SDK must not advertise scaffold case loaders."""

    assert "from_parquet" not in lv.cases.__all__
    assert not hasattr(lv.cases, "from_parquet")


__all__ = []
