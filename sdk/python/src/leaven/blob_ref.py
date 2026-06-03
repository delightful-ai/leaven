"""Public blob reference metadata records."""

from pydantic import BaseModel, ConfigDict, Field


class BlobRef(BaseModel):
    """Opaque blob reference plus stable metadata known at the seam boundary."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    blob_id: str
    sha256: str | None = None
    bytes: int | None = None
    data_classes: list[str] = Field(default_factory=list)


__all__ = ["BlobRef"]
