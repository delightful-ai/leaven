"""PromptArtifact — the simplest case: a template string the optimizer evolves."""

import json
from typing import Literal, Self

from pydantic import BaseModel, ConfigDict, Field

from .._json_parse import parse_json_object
from ..json_value import JsonObject


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


class PromptTemplateChange(BaseModel):
    """Replace the prompt template text for a prompt artifact candidate."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["replace_template"] = "replace_template"
    template: str

    def to_json_value(self) -> JsonObject:
        """Project this typed prompt change into the seam literal encoding."""
        return parse_json_object(
            json.loads(self.model_dump_json(exclude_none=True)),
            context="prompt template change",
        )


__all__ = ["PromptArtifact", "PromptTemplateChange"]
