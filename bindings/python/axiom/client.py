"""Thin, honest Python bridge to the ``axiom`` CLI.

This module exposes :class:`Axiom`, a small wrapper that shells out to the
compiled ``axiom`` binary and parses its ``--json`` output. It performs no
Axiom semantics of its own; every result is the structured payload emitted by
the CLI. If the CLI returns a non-zero exit code the call raises
:class:`axiom.errors.AxiomError`, carrying the decoded JSON payload (when
present), the exit code, and any captured stderr.

The binary is located in the following order:

1. An explicit ``binary=`` path passed to :class:`Axiom`.
2. The ``AXIOM_BIN`` environment variable.
3. ``shutil.which("axiom")`` (i.e. a binary already on ``PATH``).
4. ``<repo>/target/release/axiom``.
5. ``<repo>/target/debug/axiom``.

If none of these resolves to an existing executable, construction raises
:class:`axiom.errors.AxiomError`.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any, List, Optional

from .errors import AxiomError


def find_binary(explicit: Optional[str] = None) -> Optional[str]:
    """Return a path to the ``axiom`` binary, or ``None`` if none is found.

    Resolution order is documented on :class:`Axiom`. This variant never
    raises: it returns ``None`` when the binary cannot be located, which is
    convenient for skipping tests gracefully rather than failing them.
    """
    # 1. Explicit path.
    if explicit:
        candidate = Path(explicit).expanduser()
        if candidate.is_file():
            return str(candidate)
        return None

    # 2. Environment variable.
    env_bin = os.environ.get("AXIOM_BIN")
    if env_bin:
        candidate = Path(env_bin).expanduser()
        if candidate.is_file():
            return str(candidate)

    # 3. PATH.
    which = shutil.which("axiom")
    if which:
        return which

    # 4/5. Repo-relative target directories.
    repo_root = _find_repo_root()
    if repo_root is not None:
        for sub in ("target/release/axiom", "target/debug/axiom"):
            candidate = repo_root / sub
            if candidate.is_file():
                return str(candidate)

    return None


def _find_repo_root() -> Optional[Path]:
    """Walk up from this file until a directory containing ``Cargo.toml``."""
    here = Path(__file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "Cargo.toml").is_file():
            return parent
    return None


class Axiom:
    """Drive the ``axiom`` CLI and return its structured JSON output.

    Every public method appends ``--json`` to the underlying command and
    returns the decoded JSON object exactly as the CLI emitted it. Non-zero
    exit codes raise :class:`~axiom.errors.AxiomError`.

    Args:
        binary: Optional explicit path to the ``axiom`` binary. When omitted,
            :func:`find_binary` resolves one; if none is found, ``AxiomError``
            is raised at construction time.
    """

    def __init__(self, binary: Optional[str] = None) -> None:
        path = find_binary(binary)
        if path is None:
            hint = (
                "Could not locate the 'axiom' binary. Set the AXIOM_BIN "
                "environment variable, install 'axiom' on PATH, or build the "
                "workspace with 'cargo build -p axiom-cli'."
            )
            raise AxiomError(None, None, hint)
        self.binary = path

    # -- internal helpers -------------------------------------------------
    def _run(self, *args: str) -> Any:
        """Invoke the CLI with ``--json`` and return the parsed payload."""
        cmd: List[str] = [self.binary, *args, "--json"]
        try:
            proc = subprocess.run(cmd, capture_output=True, text=True)
        except OSError as exc:  # pragma: no cover - defensive
            raise AxiomError(None, None, f"failed to launch {self.binary}: {exc}") from exc

        stdout = (proc.stdout or "").strip()
        if proc.returncode == 0:
            if not stdout:
                return None
            try:
                return json.loads(stdout)
            except json.JSONDecodeError as exc:
                raise AxiomError(
                    None,
                    proc.returncode,
                    f"{proc.stderr}\nFailed to parse JSON output:\n{stdout}",
                ) from exc

        # Non-zero exit code: surface as AxiomError with any captured payload.
        payload: Any = None
        if stdout:
            try:
                payload = json.loads(stdout)
            except json.JSONDecodeError:
                payload = None
        raise AxiomError(payload, proc.returncode, proc.stderr or "")

    @staticmethod
    def _as_path(path: Any) -> str:
        """Accept ``str`` or ``Path`` and return a filesystem string."""
        return str(Path(path))

    # -- commands ---------------------------------------------------------
    def check(self, path: Any) -> Any:
        """Parse and type-check *path*; return the module summary."""
        return self._run("check", self._as_path(path))

    def run(
        self,
        path: Any,
        caps: Optional[List[str]] = None,
        builtin_tool: bool = False,
        emit_receipts: Optional[Any] = None,
    ) -> Any:
        """Execute *path* and return the module summary.

        ``caps`` is an optional list of capability strings (each passed as
        ``--cap <value>``). When ``emit_receipts`` is provided it is a path at
        which the CLI writes a receipt-log JSON file; the returned payload is
        still the module summary printed to stdout.
        """
        args: List[str] = ["run", self._as_path(path)]
        for cap in caps or []:
            args.append("--cap")
            args.append(str(cap))
        if builtin_tool:
            args.append("--builtin-tool")
        if emit_receipts is not None:
            args.append("--emit-receipts")
            args.append(str(Path(emit_receipts)))
        return self._run(*args)

    def verify(self, path: Any) -> Any:
        """Return which claims verify and which are blocked for *path*."""
        return self._run("verify", self._as_path(path))

    def explain(self, path: Any, claim: str) -> Any:
        """Explain why *claim* exists in *path*."""
        return self._run("explain", self._as_path(path), claim)

    def trace(self, path: Any, claim: str) -> Any:
        """Return the provenance chain of *claim* in *path*."""
        return self._run("trace", self._as_path(path), claim)

    def contradictions(self, path: Any) -> Any:
        """List contradiction witnesses for *path*."""
        return self._run("contradictions", self._as_path(path))

    def invalidate(self, path: Any, node: str) -> Any:
        """Invalidate *node* in *path* and run the incremental engine."""
        return self._run("invalidate", self._as_path(path), node)

    def replay(self, receipt_log: Any) -> Any:
        """Replay a module from a receipt-log JSON file."""
        return self._run("replay", str(Path(receipt_log)))

    def diff(self, a: Any, b: Any) -> Any:
        """Structural diff between two executed modules."""
        return self._run("diff", self._as_path(a), self._as_path(b))

    def graph(self, path: Any) -> Any:
        """Return the dependency graph (nodes, edges, dot) for *path*."""
        return self._run("graph", self._as_path(path))

    def inspect(self, path: Any, node: str) -> Any:
        """Inspect *node* by label within *path*."""
        return self._run("inspect", self._as_path(path), node)

    def conform(self, fixtures: str = "conformance") -> Any:
        """Run the conformance suite located at *fixtures* (a directory)."""
        return self._run("conform", "--fixtures", fixtures)

    def fmt(self, path: Any) -> Any:
        """Format *path* (canonical printer); report stability."""
        return self._run("fmt", self._as_path(path))

    def demo(self, which: str = "all") -> Any:
        """Run a demonstration (``1``..``7`` or ``"all"``)."""
        return self._run("demo", str(which))

    def doctor(self) -> Any:
        """Environment and self-test report."""
        return self._run("doctor")
