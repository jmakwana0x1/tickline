#!/usr/bin/env bash
# testdata/vectors/ is allowlisted in .gitleaks.toml, because fixed keys are the point of a vector
# file (#62, Jay on #72). That makes the directory a blind spot: a real private key pasted into a
# vector file is invisible to the scan. gitleaks cannot tell a 32-byte hash from a 32-byte key by
# syntax, but a field *name* can be classified, so this check says: in a vector file, a key-ish
# field may hold only a key that .gitleaks.toml already allowlists.
#
# The allowlist is read from .gitleaks.toml rather than copied, so the two cannot drift.
#
# Usage: check-vector-secrets.sh [vectors-dir] [gitleaks-config]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
vectors="${1:-$root/testdata/vectors}"
config="${2:-$root/.gitleaks.toml}"

python3 - "$vectors" "$config" <<'PY'
import json, re, sys
from pathlib import Path

vectors, config = Path(sys.argv[1]), Path(sys.argv[2])

# Field names that may hold key material. Short tokens match a whole segment, so "sk" matches
# "signerSk" and "sk" but not "risk"; long ones match anywhere, so no spelling sidesteps them.
SEGMENT_TOKENS = {"sk", "priv", "key", "keys"}
SUBSTRING_TOKENS = ("private", "privkey", "secret", "seed", "mnemonic", "passphrase")

def segments(name):
    parts = re.split(r"[_\-\s]+", re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name))
    return {p.lower() for p in parts if p}

def is_keyish(name):
    low = name.lower()
    return bool(segments(name) & SEGMENT_TOKENS) or any(t in low for t in SUBSTRING_TOKENS)

def allowed_keys(text):
    # The [allowlist] regexes block of .gitleaks.toml. Entries that are a bare 64-hex literal are
    # the public anvil keys; anything else there is a pattern and is not a literal we can compare.
    block = text.split("[allowlist]", 1)
    if len(block) != 2:
        raise SystemExit("✗ .gitleaks.toml has no [allowlist] section")
    return {m.lower() for m in re.findall(r"'''([0-9a-fA-F]{64})'''", block[1])}

def leaves(node, path):
    if isinstance(node, dict):
        for k, v in node.items():
            yield from leaves(v, f"{path}.{k}")
    elif isinstance(node, list):
        for i, v in enumerate(node):
            yield from leaves(v, f"{path}[{i}]")
    else:
        yield path, node

def walk(node, path, problems, allowed, file):
    if isinstance(node, dict):
        for k, v in node.items():
            here = f"{path}.{k}"
            if is_keyish(k):
                # The other shape a key takes in JSON: 32 or 64 byte values in an array. The
                # length is the tell, whatever the values are (Jay on #72).
                if isinstance(v, list) and len(v) in (32, 64) and all(isinstance(x, int) for x in v):
                    problems.append(f"{file}: {here} is a key-ish field holding {len(v)} byte values")
                    continue
                for leaf_path, leaf in leaves(v, here):
                    # Only strings can carry key material: a 256-bit key is not a JSON number,
                    # and lmsr.json's `provenance.seed` is the PRNG seed 402, not a secret.
                    if not isinstance(leaf, str):
                        continue
                    value = leaf.lower().removeprefix("0x")
                    if value not in allowed:
                        problems.append(f"{file}: {leaf_path} is a key-ish field whose value is not an allowlisted anvil key")
            else:
                walk(v, here, problems, allowed, file)
    elif isinstance(node, list):
        for i, v in enumerate(node):
            walk(v, f"{path}[{i}]", problems, allowed, file)

allowed = allowed_keys(config.read_text())
if not allowed:
    raise SystemExit("✗ no anvil keys found in .gitleaks.toml; the allowlist cannot be read")

files = sorted(vectors.rglob("*.json"))
problems = []
for file in files:
    try:
        data = json.loads(file.read_text())
    except json.JSONDecodeError as e:
        problems.append(f"{file}: not valid JSON ({e})")
        continue
    walk(data, "$", problems, allowed, file)

for p in problems:
    print(f"✗ {p}", file=sys.stderr)
if problems:
    sys.exit(1)
print(f"✓ no unlisted key material in {len(files)} vector file(s), against {len(allowed)} allowlisted anvil key(s)")
PY
