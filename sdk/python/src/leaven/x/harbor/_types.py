"""Typed Harbor rollout evidence records used by rollout and reward helpers."""

import json
from typing import Self

from pydantic import BaseModel, ConfigDict, Field, ValidationError, model_validator


class HarborAdapterError(RuntimeError):
    """Actionable error raised at the optional Harbor adapter boundary."""


class CtrfEvidence(BaseModel):
    """Partial-credit evidence from a CTRF report."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    passed: int = 0
    failed: int = 0
    total: int = 0
    failed_names: list[str] = Field(default_factory=list)


class TokenEvidence(BaseModel):
    """Token totals reported by a Harbor trial."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    input: int | None = None
    output: int | None = None


class HarborTrialOutcome(BaseModel):
    """Stable structured rollout evidence for one Harbor trial."""

    model_config = ConfigDict(frozen=True, extra="ignore")

    trial_dir: str | None = None
    rewards: dict[str, float] = Field(default_factory=dict)
    ctrf: CtrfEvidence | None = None
    verifier_output: str = ""
    trajectory_path: str | None = None
    tokens: TokenEvidence | None = None
    cost_usd: float | None = None
    exception: str | None = None

    @model_validator(mode="before")
    @classmethod
    def _accept_legacy_terminal_bench_shape(cls, value: object) -> object:
        if not isinstance(value, dict):
            return value
        data = dict(value)
        if "reward" in data and "rewards" not in data:
            data["rewards"] = {"reward": data.pop("reward")}
        ctrf_keys = {"ctrf_passed", "ctrf_total"}
        if ctrf_keys.intersection(data) and "ctrf" not in data:
            passed = int(data.pop("ctrf_passed", 0) or 0)
            total = int(data.pop("ctrf_total", 0) or 0)
            data["ctrf"] = {
                "passed": passed,
                "failed": max(total - passed, 0),
                "total": total,
            }
        if ("input_tokens" in data or "output_tokens" in data) and "tokens" not in data:
            data["tokens"] = {
                "input": data.pop("input_tokens", None),
                "output": data.pop("output_tokens", None),
            }
        return data

    @property
    def ctrf_fraction(self) -> float:
        if self.ctrf is None or self.ctrf.total <= 0:
            return 0.0
        return self.ctrf.passed / self.ctrf.total

    @property
    def reward(self) -> float:
        return float(self.rewards.get("reward", 0.0))

    @property
    def ctrf_passed(self) -> int:
        return 0 if self.ctrf is None else self.ctrf.passed

    @property
    def ctrf_total(self) -> int:
        return 0 if self.ctrf is None else self.ctrf.total

    @property
    def input_tokens(self) -> int | None:
        return None if self.tokens is None else self.tokens.input

    @property
    def output_tokens(self) -> int | None:
        return None if self.tokens is None else self.tokens.output

    def encode(self) -> str:
        return self.model_dump_json(exclude_none=True)

    @classmethod
    def decode(cls, data: str | bytes) -> Self:
        try:
            raw = data.decode() if isinstance(data, bytes) else data
            return cls.model_validate(json.loads(raw))
        except (json.JSONDecodeError, TypeError, ValidationError) as exc:
            raise HarborAdapterError(f"invalid Harbor trial outcome JSON: {exc}") from exc


__all__ = [
    "CtrfEvidence",
    "HarborAdapterError",
    "HarborTrialOutcome",
    "TokenEvidence",
]
