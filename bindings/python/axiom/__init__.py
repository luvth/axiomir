"""Axiom IR — Python bridge to the ``axiom`` CLI.

This package is a thin, honest wrapper around the compiled ``axiom`` binary.
It shells out to the CLI, requests ``--json`` output, and returns the decoded
payloads. It deliberately reimplements none of Axiom's semantics.

Typical usage::

    from axiom import Axiom
    client = Axiom()
    summary = client.check("my_module.axiom")
    print(summary["verified"])

See :class:`axiom.client.Axiom` for the full command surface and
:class:`axiom.errors.AxiomError` for the failure type.
"""

from __future__ import annotations

from .client import Axiom, find_binary
from .errors import AxiomError

__all__ = ["Axiom", "AxiomError", "find_binary"]
