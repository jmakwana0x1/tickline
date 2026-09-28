#!/usr/bin/env bash
# Every text file type the gitleaks evm-private-key rule does not scan (#81, #85, found by Jay).
#
# That rule's path filter is (rs|ts|tsx|sol|json|ya?ml|env|sh|toml). Everything else is invisible to
# it, and gitleaks' default rules only catch a bare hex when a keyword sits beside it. Tracked types
# outside the filter today: md (30 files), extensionless (10), lock (2), txt (2), example (1), sql
# (1), js (1), py (1).
#
# sql is the one that will matter: Phase 4's migrations are where a seeded fixture or a connection
# string would land. md is the one that matters now, because it is what this repository writes most.
#
# Patching each type with its own script would be whack-a-mole, so this covers all of them with one
# rule: a 64-hex value is flagged when a key-ish word sits within three lines of it. Key-ish comes
# from scripts/keyish.py, the single definition this repo has, after the first version of this file
# reimplemented it as a plain \b regex and missed both private_key and privateKey.
#
# Three lines rather than the same line, because a pasted key sits under its label or inside a fenced
# block. The anvil keys are read from .gitleaks.toml so the two cannot drift.
#
# Usage: check-text-secrets.sh [root]   (default: this repository)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="${1:-$(cd "$here/.." && pwd -P)}"
[[ -d "$root" ]] || { echo "✗ no such directory: $root" >&2; exit 2; }

PYTHONPATH="$here" python3 - "$root" "$(cd "$here/.." && pwd -P)/.gitleaks.toml" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

from keyish import text_has_keyish_word

root, config = Path(sys.argv[1]), Path(sys.argv[2])

WINDOW = 3
HEX = re.compile(r"\b(?:0x)?([0-9a-fA-F]{64})\b")

# What the gitleaks rule already scans. Anything else here is ours to cover.
SCANNED_BY_GITLEAKS = {
    ".rs", ".ts", ".tsx", ".sol", ".json", ".yml", ".yaml", ".env", ".sh", ".toml"
}

# Never walked: not ours, and a flagged dependency README is a confusing failure.
SKIP_DIRS = {"node_modules", "lib", "target", "out", "cache", ".git", "dist"}


def allowed() -> set[str]:
    """Every 64-hex literal .gitleaks.toml allows, read rather than copied.

    Not only the anvil keys: the market id tag and Pyth's feed id are in there too, each with its
    source. Reading the file rather than hardcoding a list is what keeps the two from disagreeing.
    """
    text = config.read_text() if config.exists() else ""
    parts = text.split("[allowlist]", 1)
    return {m.lower() for m in re.findall(r"'''([0-9a-fA-F]{64})'''", parts[1] if len(parts) == 2 else "")}


def candidates() -> list[Path]:
    """Tracked files the rule does not scan, or every file when the root is not a git tree."""
    listed = subprocess.run(
        ["git", "-C", str(root), "ls-files"], capture_output=True, text=True
    )
    if listed.returncode == 0 and listed.stdout.strip():
        paths = [root / name for name in listed.stdout.split()]
    else:
        paths = [p for p in root.rglob("*") if not SKIP_DIRS & set(p.relative_to(root).parts)]
    return [
        p for p in paths
        if p.is_file() and p.suffix.lower() not in SCANNED_BY_GITLEAKS
        and not SKIP_DIRS & set(p.relative_to(root).parts)
    ]


keys = allowed()
problems = []
scanned = 0
for path in sorted(candidates()):
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (UnicodeDecodeError, OSError):
        continue  # binary, or unreadable: nothing a text rule can say about it
    scanned += 1
    for index, line in enumerate(lines):
        for match in HEX.finditer(line):
            if match.group(1).lower() in keys:
                continue
            lo, hi = max(0, index - WINDOW), min(len(lines), index + WINDOW + 1)
            near = text_has_keyish_word("\n".join(lines[lo:hi]))
            if near:
                problems.append(
                    f"{path}:{index + 1}: a 32-byte hex value within {WINDOW} lines of '{near}'"
                )

for problem in problems:
    print(f"✗ {problem}", file=sys.stderr)
if problems:
    print(
        "  gitleaks does not scan this file type, so this check is the only thing between a pasted\n"
        "  key and the repository. If the value is public test material, add it to the allowlist in\n"
        "  .gitleaks.toml with its source, the way the anvil keys are.",
        file=sys.stderr,
    )
    sys.exit(1)
print(f"✓ no key-shaped value near a key-ish word in {scanned} unscanned text files")
PY
