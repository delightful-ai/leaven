"""PromptArtifact — the simplest case: a template string the optimizer evolves."""

from typing import Self

from pydantic import BaseModel, ConfigDict, Field


class PromptArtifact(BaseModel):
    """A prompt template plus optional few-shot examples.

    Template uses Python `.format(**case.input)` substitution by convention.
    Optimizers evolve the template string; examples may also be evolved
    depending on the optimizer.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    template: str
    """The prompt template. `{var}` placeholders bind to case input keys."""

    examples: list[str] = Field(default_factory=list)
    """Few-shot examples prepended to the rendered prompt, in order."""

    candidate_id: str | None = None
    """Set when this artifact came from the engine; None for hand-built seeds."""

    @classmethod
    def empty(cls) -> Self:
        """An empty seed artifact (template = empty string)."""
        return cls(template="")


__all__ = ["PromptArtifact"]
