"""Console entry point for the ``axiom-py`` script.

Running ``axiom-py`` performs a quick self-check: it locates the ``axiom``
binary, runs ``doctor``, and prints the JSON report. This is a convenience
wrapper only — all real work is delegated to the CLI.
"""

from __future__ import annotations

import json
import sys

from .client import Axiom, find_binary
from .errors import AxiomError


def main(argv=None) -> int:  # noqa: ARG001 - argv reserved for future use
    """Locate the binary, run ``doctor``, and print the report.

    Returns:
        0 on success, 2 when the binary cannot be located, otherwise the
        CLI's own non-zero exit code.
    """
    binary = find_binary()
    if binary is None:
        sys.stderr.write(
            "axiom-py: could not locate the 'axiom' binary. Set AXIOM_BIN, "
            "put 'axiom' on PATH, or run 'cargo build -p axiom-cli'.\n"
        )
        return 2
    try:
        client = Axiom(binary=binary)
        report = client.doctor()
    except AxiomError as exc:
        sys.stderr.write(f"axiom-py: {exc}\n")
        return exc.exit_code or 1
    sys.stdout.write(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
