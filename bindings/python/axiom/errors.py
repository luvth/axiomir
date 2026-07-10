"""Exception types for the Axiom IR Python bridge.

This module defines :class:`AxiomError`, the single failure type raised when
the underlying ``axiom`` CLI binary returns a non-zero exit code (or otherwise
fails to produce a usable JSON payload). The bridge is intentionally thin and
honest: it never reimplements Axiom semantics, it only surfaces what the CLI
reports.
"""

from __future__ import annotations

from typing import Any, Optional


class AxiomError(Exception):
    """Raised when an ``axiom`` CLI invocation fails.

    The bridge is a thin, honest wrapper. When the CLI returns a non-zero exit
    code the call raises :class:`AxiomError`, carrying as much information as
    the CLI made available so callers can inspect the failure.

    Attributes:
        json: The parsed JSON payload returned by the CLI, if any could be
            decoded. ``None`` when the CLI produced no parseable output.
        exit_code: The process exit code (``0`` = ok, ``1`` = failure,
            ``2`` = usage), or ``None`` when the process could not be launched
            at all.
        stderr: Captured standard-error text from the CLI invocation.
    """

    def __init__(
        self,
        json: Any,
        exit_code: Optional[int],
        stderr: str,
        message: Optional[str] = None,
    ) -> None:
        self.json = json
        self.exit_code = exit_code
        self.stderr = stderr or ""
        if message is None:
            parts: list = []
            if exit_code is not None:
                parts.append(f"axiom exited with code {exit_code}")
            if self.stderr:
                parts.append(self.stderr.strip())
            if json is not None:
                parts.append(f"payload: {json}")
            message = "; ".join(parts) if parts else "axiom invocation failed"
        super().__init__(message)

    def __str__(self) -> str:
        return super().__str__()
