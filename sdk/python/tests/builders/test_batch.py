"""Tests for the private `leaven.builders.batch` placeholder."""

import importlib
import inspect
from types import ModuleType

import pytest

from leaven.builders.batch import _batch, _BatchBuilder

batch_module = importlib.import_module("leaven.builders.batch")
builders_module = importlib.import_module("leaven.builders")


def test_batch_module_exports_no_public_placeholder() -> None:
    assert batch_module.__all__ == []
    assert "BatchBuilder" not in batch_module.__dict__
    assert "batch" not in batch_module.__dict__


def test_builders_namespace_does_not_export_batch_surface() -> None:
    assert "BatchBuilder" not in builders_module.__all__
    assert "batch" not in builders_module.__all__
    assert "BatchBuilder" not in builders_module.__dict__
    assert isinstance(builders_module.__dict__["batch"], ModuleType)


def test_batch_module_does_not_use_public_not_implemented_scaffold() -> None:
    assert "NotImplementedError" not in inspect.getsource(batch_module)


async def test_private_batch_builder_refuses_runtime_use() -> None:
    builder = _BatchBuilder()

    with pytest.raises(RuntimeError, match="private batch placeholder"):
        await builder.__aenter__()


def test_private_batch_factory_refuses_runtime_use() -> None:
    with pytest.raises(RuntimeError, match="private batch placeholder"):
        _batch()
