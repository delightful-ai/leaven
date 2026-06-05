"""Private SDK error types shared by public-facing modules."""


class UnboundBuilderError(RuntimeError):
    """A role-scoped builder was used outside its engine-bound stage context."""


__all__ = ["UnboundBuilderError"]
