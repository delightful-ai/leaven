"""Private error types for the Leaven public seam process client."""


class SeamClientError(RuntimeError):
    """Raised when the public seam process cannot be driven successfully."""


__all__ = ["SeamClientError"]
