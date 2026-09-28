#!/usr/bin/env python3
"""One definition of "key-ish", because two that disagree is worse than one that is wrong (#85).

`check-vector-secrets.sh` had this logic and `check-md-secrets.sh` reimplemented it as a plain `\\b`
regex, which lost the half that matters: `_` is a word character, so `\\bprivate\\b` does not match
`private_key`, and a camelCase `privateKey` does not match either. Six of the eight spellings a wallet
export or a `.env` line actually produces were missed, and the two that matched were the ones nobody
writes (Jay, on #85).

The rule, unchanged from the version that was right:

* names are split into segments on `_`, `-`, spaces and camelCase boundaries;
* **short** tokens match a whole segment, so `sk` matches `signerSk` and `sk` but not `risk`;
* **long** tokens match anywhere, so no spelling of `private` or `secret` can slip past.
"""

import re

#: Matched against a whole segment. Short enough that a substring match would fire on ordinary words.
SEGMENT_TOKENS = frozenset({"sk", "pk", "priv", "key", "keys"})

#: Matched anywhere in the name. Long enough that a substring match is safe.
SUBSTRING_TOKENS = ("private", "privkey", "secret", "seed", "mnemonic", "passphrase")

_SPLIT = re.compile(r"[_\-\s.]+")
_CAMEL = re.compile(r"(?<=[a-z0-9])(?=[A-Z])")

#: An identifier-ish run of characters, for scanning prose rather than a field name.
WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_\-]*")


def segments(name: str) -> set[str]:
    """The lowercase segments of a name, split on separators and camelCase boundaries."""
    return {part.lower() for part in _SPLIT.split(_CAMEL.sub("_", name)) if part}


def is_keyish(name: str) -> bool:
    """True when `name` looks like it holds key material."""
    lowered = name.lower()
    return bool(segments(name) & SEGMENT_TOKENS) or any(t in lowered for t in SUBSTRING_TOKENS)


def text_has_keyish_word(text: str) -> str | None:
    """The first key-ish word in free text, or None.

    Prose is scanned word by word rather than with one regex over the whole string, so the same
    segmentation applies to `private_key` in a sentence as to `private_key` as a JSON field.
    """
    for match in WORD.finditer(text):
        if is_keyish(match.group()):
            return match.group()
    return None
