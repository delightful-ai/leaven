"""`lv.optimize(...).run()` — the entry point for an optimization run.

Composes a seed artifact, an environment (task + rollout + rubric), an
optimizer config, and a runtime into a runnable optimization. The builder is
typed by the artifact type so `result.best.artifact` is fully typed. Train /
validation / test splits come from `Case.split` tags on the environment's task.

`.run()` for the current prompt mechanics path uses the durable
`leaven seam serve --stdio` server route and sends locked runner/proposer
`leaven/stage.run` requests through private `_seam` client machinery. The
current prompt slice dispatches registered Python runner stages and configured
`Propose.fn(...)` proposer stages through checked-in subprocess workers. The
proposer path submits a proposal batch only; proposal application, admission,
and real optimizer search remain later slices.
"""

from typing import cast

from ._errors import UnsupportedConfigurationError
from ._runs import persist_rust_prompt_checkpoint
from ._seam_optimize import PlannedOptimizeCase, SeamOptimizeReport, run_prompt_mechanics
from .artifacts.prompt import PromptArtifact
from .decorators import RegisteredStage
from .environment import Environment
from .optimizers.config import OptimizerConfig
from .optimizers.gepa import Gepa
from .result import Optimized
from .runtime import Runtime


class OptimizeBuilder[A]:
    """Builder returned by `lv.optimize(...)`. Call `.run()` to execute."""

    seed: A
    environment: Environment
    optimizer: OptimizerConfig
    runtime: Runtime

    async def run(self) -> Optimized[A]:
        """Execute the optimization. Returns when the optimizer terminates.

        Termination conditions (whichever comes first): optimizer-internal
        stopping criteria, budget exhausted, max iterations reached, user
        cancellation.
        """
        seed = self._prompt_seed()
        runner = self._runner_stage()
        optimizer = self._gepa_config()
        cases = self._plan_cases()
        run_id = self._run_id()

        report = await run_prompt_mechanics(
            seed=seed,
            cases=cases,
            runner=runner,
            optimizer=optimizer,
            rubric=self.environment.rubric,
            run_id=run_id,
            runtime=self.runtime,
        )
        return cast("Optimized[A]", _to_optimized(seed, cases, report, run_id))

    def _prompt_seed(self) -> PromptArtifact:
        if not isinstance(self.seed, PromptArtifact):
            raise TypeError(
                f"this slice optimizes a PromptArtifact seed; got {type(self.seed).__name__}"
            )
        return self.seed

    def _runner_stage(self) -> RegisteredStage[object, object]:
        rollout = self.environment.rollout
        if rollout.kind != "function" or rollout.stage is None:
            raise UnsupportedConfigurationError(
                "this slice supports a function rollout (`Rollout.fn(runner)`); "
                f"got rollout kind {rollout.kind!r}"
            )
        if rollout.stage.role != "runner":
            raise TypeError(
                f"the rollout stage must be a @lv.runner; got role {rollout.stage.role!r}"
            )
        return rollout.stage

    def _gepa_config(self) -> Gepa:
        if not isinstance(self.optimizer, Gepa):
            raise UnsupportedConfigurationError(
                "this slice supports the GEPA optimizer (`lv.optimizers.gepa(...)`); "
                f"got optimizer {self.optimizer.name!r}"
            )
        return self.optimizer

    def _plan_cases(self) -> list[PlannedOptimizeCase]:
        cases = self.environment.task.cases
        if not cases:
            raise ValueError("the task has no cases to optimize over")
        return [
            PlannedOptimizeCase(
                case_id=_wire_case_id(case.id),
                input=dict(case.input),
                target=dict(case.target) if case.target is not None else None,
                metadata=dict(case.metadata),
                split=case.split,
            )
            for case in cases
        ]

    def _run_id(self) -> str:
        name = self.environment.task.name
        return _slug(name) if name else "leaven_optimize"


def optimize[A](
    *,
    seed: A,
    environment: Environment,
    optimizer: OptimizerConfig,
    runtime: Runtime,
) -> OptimizeBuilder[A]:
    """Compose an optimization run. Call `.run()` to execute.

    `environment` bundles the task (cases with split tags, sandbox needs), the
    rollout (how the current artifact runs on a case), and the rubric (how the
    result scores). The optimizer owns the outer loop (reflect / propose /
    judge). Train / validation / test splits are read from `Case.split` on the
    environment's task; the runtime supplies execution, budget, and trust.
    """
    b = OptimizeBuilder[A]()
    b.seed = seed
    b.environment = environment
    b.optimizer = optimizer
    b.runtime = runtime
    return b


def _to_optimized(
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    report: SeamOptimizeReport,
    run_id: str,
) -> Optimized[PromptArtifact]:
    """Materialize the durable-seam mechanics report through Rust readback."""
    return persist_rust_prompt_checkpoint(seed=seed, cases=cases, report=report, run_id=run_id)


def _wire_case_id(case_id: str) -> str:
    """Project a user case id into the wire CaseId pattern `^case_[A-Za-z0-9_.:-]+$`.

    Hyphens are the only common id character outside the wire pattern's body, so
    they map to underscores; the `case_` prefix is added when absent.
    """
    body = case_id.replace("-", "_")
    return body if body.startswith("case_") else f"case_{body}"


def _slug(name: str) -> str:
    """Lower a free-form name into a run-id-safe slug."""
    cleaned = "".join(ch if ch.isalnum() else "_" for ch in name).strip("_")
    return cleaned or "leaven_optimize"


__all__ = ["OptimizeBuilder", "optimize"]
