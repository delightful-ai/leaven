"""Private SDK error types shared by public-facing modules."""


class UnboundBuilderError(RuntimeError):
    """A role-scoped builder was used outside its engine-bound stage context."""


class UnsupportedConfigurationError(RuntimeError):
    """A requested SDK configuration is not part of the current wired product surface."""


__all__ = ["UnboundBuilderError", "UnsupportedConfigurationError"]
