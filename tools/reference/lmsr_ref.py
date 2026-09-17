#!/usr/bin/env python3
"""Reference LMSR, computed with mpmath, written to testdata/vectors/lmsr.json.

This is the independent oracle the Rust ``lmsr`` crate is diffed against (issue #38). It is
deliberately the plain transcription of the formulas in PHASES.md phase 1: its only job is to be
right, so readability beats speed.

Units follow CLAUDE.md and Jay's decisions on #31:

* every quantity is a signed WAD integer (1e18 = 1.0);
* one share pays 1 USDC, so share and USDC base units are 1e-6 and a WAD value converts to base
  units by dividing by 1e12;
* ``b`` lies in [B_MIN, B_MAX] = [1, 1e7] shares and each outcome's quantity in [0, Q_MAX] =
  [0, 1e12] shares (D2).

Every real value is stored as ``floor`` (the exact floor, as a decimal string) plus ``inexact``
(whether the true value is not an integer), so a consumer can derive the ceiling and check its
rounding direction exactly. Conversions to base units are stored already rounded toward the
vault: costs up, shares down (I15).

Precision. Each value is computed with at least PRECISION_DIGITS significant digits and at least
GUARD_DIGITS digits below the wei, then recomputed with EXTRA_DIGITS more, and the generator
refuses to write a value whose floor or integrality changes between the two. The guard matters
for exp: its domain reaches values near 1e76 wei, which 60 significant digits alone cannot floor.

Determinism. Inputs come from SplitMix64 with a fixed seed, not from Python's ``random``, and
the file is written with a fixed layout, so ``just vectors-lmsr`` reproduces it byte for byte.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

import mpmath
from mpmath import mp, mpf

SEED = 402
PRECISION_DIGITS = 60
GUARD_DIGITS = 30
EXTRA_DIGITS = 20
SCHEMA = 1

WAD = 10**18
BASE_UNIT_SCALE = 10**12  # WAD -> USDC or share base units (1e-6)
B_MIN = 10**18
B_MAX = 10**25
Q_MAX = 10**30
U128_MAX = 2**128 - 1
I256_MAX = 2**255 - 1

# Solady FixedPointMathLib at 9fe23ffdcd395c4169396064226ed27206b222c3, expWad:
# returns 0 for x <= EXP_ZERO_AT, reverts for x >= EXP_OVERFLOW_AT.
EXP_ZERO_AT = -41446531673892822313
EXP_OVERFLOW_AT = 135305999368893231589

GROUP_SIZES = {
    "exp_wad": 1100,
    "ln_wad": 1100,
    "cost": 900,
    "price": 700,
    "cost_to_buy": 1300,
    "subsidy": 150,
}


class SplitMix64:
    """Tiny, fully specified PRNG, so the vectors never depend on Python's ``random``."""

    MASK = 2**64 - 1

    def __init__(self, seed: int) -> None:
        self.state = seed & self.MASK

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & self.MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & self.MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & self.MASK
        return z ^ (z >> 31)

    def below(self, n: int) -> int:
        """Uniform integer in [0, n), by rejection, for n up to 2**256."""
        if n <= 0:
            raise ValueError("n must be positive")
        bits = n.bit_length()
        while True:
            words = (bits + 63) // 64
            v = 0
            for _ in range(words):
                v = (v << 64) | self.next()
            v >>= words * 64 - bits
            if v < n:
                return v

    def between(self, lo: int, hi: int) -> int:
        """Uniform integer in [lo, hi]."""
        return lo + self.below(hi - lo + 1)

    def log_uniform(self, lo: int, hi: int) -> int:
        """Integer in [lo, hi] (lo >= 1) whose magnitude is spread across orders of magnitude."""
        lo_bits, hi_bits = lo.bit_length(), hi.bit_length()
        bits = self.between(lo_bits, hi_bits)
        top = min(hi, 2**bits - 1)
        bottom = max(lo, 2 ** (bits - 1))
        if bottom > top:
            return lo
        return self.between(bottom, top)

    def choice(self, items: list):
        return items[self.below(len(items))]


def real(fn, *, int_digits_hint: int) -> tuple[int, bool]:
    """Evaluate ``fn()`` (a WAD-scaled mpf) twice at rising precision; return (floor, inexact).

    Refuses to return a value whose floor or integrality differs between the two precisions.
    """
    results = []
    base = max(PRECISION_DIGITS, int_digits_hint + GUARD_DIGITS)
    for dps in (base, base + EXTRA_DIGITS):
        with mp.workdps(dps):
            v = fn()
            f = int(mpmath.floor(v))
            results.append((f, v != f))
    if results[0] != results[1]:
        raise AssertionError(f"floor is not stable across precisions: {results}")
    return results[0]


def digits(n: int) -> int:
    return len(str(abs(n))) + 1


# ------------------------------------------------------------------ math (exact, in real units)


def exp_wad(x: int) -> tuple[int, bool]:
    # exp(x / 1e18) * 1e18; an upper bound on the result's size sets the working precision.
    hint = max(1, int(x // WAD * 0.4343) + 19) if x > 0 else 19
    return real(lambda: mpmath.exp(mpf(x) / WAD) * WAD, int_digits_hint=hint)


def ln_wad(x: int) -> tuple[int, bool]:
    return real(lambda: mpmath.log(mpf(x) / WAD) * WAD, int_digits_hint=digits(x) + 2)


def tail(delta: int, b: int) -> mpf:
    """b * ln(1 + exp(-|delta| / b)), in WAD, with b and delta WAD integers.

    Everything is kept in WAD units so no WAD division enters: |delta| / b is a ratio of
    integers, and max(qY, qN), the integer part of the cost, never passes through mpmath.
    """
    return mpf(b) * mpmath.log1p(mpmath.exp(-mpf(abs(delta)) / b))


def check_log_sum_exp(q_yes: int, q_no: int, b: int) -> None:
    """The log-sum-exp form must equal the naive form, which mpmath can evaluate directly."""
    lse = max(q_yes, q_no) + tail(q_yes - q_no, b)
    naive = mpf(b) * mpmath.log(mpmath.exp(mpf(q_yes) / b) + mpmath.exp(mpf(q_no) / b))
    if abs(lse - naive) > mpf(10) ** (-(mp.dps - 12)) * max(1, abs(lse)):
        raise AssertionError(f"log-sum-exp and naive cost disagree for {(q_yes, q_no, b)}")


def cost(q_yes: int, q_no: int, b: int) -> tuple[int, bool]:
    """C(qY, qN) in WAD. The tail is irrational and positive, so the cost is never an integer."""

    def fractional():
        check_log_sum_exp(q_yes, q_no, b)
        return tail(q_yes - q_no, b)

    tail_floor, _ = real(fractional, int_digits_hint=digits(b))
    return max(q_yes, q_no) + tail_floor, True


def price(q_yes: int, q_no: int, b: int) -> dict:
    """pY = 1 / (1 + exp((qN - qY) / b)) and pN = 1 - pY, in WAD.

    The smaller price is computed directly and the larger derived in exact integers, because
    1 - tiny rounds to exactly 1 at any fixed precision once tiny < 10^-precision, at both of
    the precisions the stability check compares.
    """
    diff = q_no - q_yes  # pY is the smaller price when diff > 0

    def small():
        e = mpmath.exp(-mpf(abs(diff)) / b)
        return e / (1 + e) * WAD

    s_floor, s_inexact = real(small, int_digits_hint=19)
    s_ceil = s_floor + (1 if s_inexact else 0)
    large = (WAD - s_ceil, s_inexact)  # floor(WAD - s) = WAD - ceil(s)
    smaller = (s_floor, s_inexact)
    py, pn = (smaller, large) if diff > 0 else (large, smaller)
    if diff == 0 and (py[0] != WAD // 2 or py[1]):
        raise AssertionError("price at balance must be exactly one half")
    return {"yes_floor": str(py[0]), "yes_inexact": py[1], "no_floor": str(pn[0]), "no_inexact": pn[1]}


def cost_to_buy(q_yes: int, q_no: int, b: int, outcome: str, d: int) -> tuple[int, bool]:
    """C(after) - C(before) in WAD: exact integer parts plus the difference of the tails."""
    after_yes, after_no = (q_yes + d, q_no) if outcome == "yes" else (q_yes, q_no + d)
    integer_part = max(after_yes, after_no) - max(q_yes, q_no)

    def fractional():
        return tail(after_yes - after_no, b) - tail(q_yes - q_no, b)

    f, inexact = real(fractional, int_digits_hint=digits(b))
    value = (integer_part + f, inexact)
    if d > 0 and (value[0] < 0 or (value[0] == 0 and not value[1])):
        raise AssertionError(f"cost to buy is not positive for {(q_yes, q_no, b, outcome, d)}")
    return value


def ceil_div(a: int, b: int) -> int:
    return -((-a) // b)


# ------------------------------------------------------------------ case generation


def fields(**kw) -> dict:
    return {k: (str(v) if isinstance(v, int) and not isinstance(v, bool) else v) for k, v in kw.items()}


def exp_cases(rng: SplitMix64) -> list[dict]:
    xs = [
        0, 1, -1, WAD, -WAD, 693147180559945309, -693147180559945309,
        EXP_ZERO_AT, EXP_ZERO_AT + 1, EXP_ZERO_AT - 1,
        EXP_OVERFLOW_AT - 1, EXP_OVERFLOW_AT, EXP_OVERFLOW_AT + 1,
        -(2**255), I256_MAX,
    ]
    while len(xs) < GROUP_SIZES["exp_wad"]:
        pick = rng.below(4)
        if pick == 0:
            xs.append(rng.between(EXP_ZERO_AT, EXP_OVERFLOW_AT - 1))
        elif pick == 1:
            xs.append(-rng.log_uniform(1, -EXP_ZERO_AT))  # the log-sum-exp form only needs x <= 0
        elif pick == 2:
            xs.append(rng.log_uniform(1, EXP_OVERFLOW_AT - 1))
        else:
            xs.append(rng.between(-(2 * WAD), 2 * WAD))
    out = []
    for x in xs:
        if x >= EXP_OVERFLOW_AT:
            out.append(fields(x=x, error="overflow"))
            continue
        f, inexact = exp_wad(x)
        region = "solady_returns_zero" if x <= EXP_ZERO_AT else "in_domain"
        out.append(fields(x=x, floor=f, inexact=inexact, region=region))
    return out


def ln_cases(rng: SplitMix64) -> list[dict]:
    xs = [1, 2, WAD - 1, WAD, WAD + 1, 2 * WAD, 10**36, I256_MAX, U128_MAX, 0, -1, -(2**255)]
    while len(xs) < GROUP_SIZES["ln_wad"]:
        if rng.below(3) == 0:
            xs.append(rng.between(WAD // 2, 4 * WAD))  # 1 + exp(...) lies in (1, 2] WAD
        else:
            xs.append(rng.log_uniform(1, I256_MAX))
    out = []
    for x in xs:
        if x <= 0:
            out.append(fields(x=x, error="undefined"))
            continue
        f, inexact = ln_wad(x)
        out.append(fields(x=x, floor=f, inexact=inexact))
    return out


def lmsr_inputs(rng: SplitMix64, count: int) -> list[tuple[int, int, int]]:
    """(qY, qN, b) inside the D2 bounds, edges first."""
    edges = [
        (0, 0, B_MIN), (0, 0, B_MAX), (0, 0, WAD * 100),
        (WAD, WAD, B_MIN), (Q_MAX, Q_MAX, B_MIN), (Q_MAX, Q_MAX, B_MAX),
        (Q_MAX, 0, B_MIN), (0, Q_MAX, B_MIN), (Q_MAX, 0, B_MAX), (0, Q_MAX, B_MAX),
        (1, 0, B_MIN), (0, 1, B_MAX), (WAD * 50, WAD * 49, WAD * 10),
    ]
    out = list(edges)
    while len(out) < count:
        b = rng.choice([B_MIN, B_MAX, rng.log_uniform(B_MIN, B_MAX)])
        shape = rng.below(4)
        if shape == 0:  # symmetric
            q = rng.choice([0, Q_MAX, rng.log_uniform(1, Q_MAX)])
            out.append((q, q, b))
        elif shape == 1:  # small imbalance relative to b
            base = rng.log_uniform(1, Q_MAX - 1000 * B_MAX)
            gap = rng.between(0, 20 * b)
            out.append((base + gap, base, b) if rng.below(2) else (base, base + gap, b))
        elif shape == 2:  # large imbalance
            hi = rng.choice([Q_MAX, rng.log_uniform(WAD, Q_MAX)])
            out.append((hi, rng.between(0, hi // 1000), b) if rng.below(2) else (rng.between(0, hi // 1000), hi, b))
        else:
            out.append((rng.between(0, Q_MAX), rng.between(0, Q_MAX), b))
    return out[:count]


def cost_cases(rng: SplitMix64) -> list[dict]:
    out = []
    for q_yes, q_no, b in lmsr_inputs(rng, GROUP_SIZES["cost"] - 8):
        f, inexact = cost(q_yes, q_no, b)
        out.append(fields(q_yes=q_yes, q_no=q_no, b=b, floor=f, inexact=inexact))
    # Out-of-domain inputs the implementation must reject.
    for q_yes, q_no, b, error in [
        (0, 0, 0, "liquidity_not_positive"),
        (0, 0, -WAD, "liquidity_not_positive"),
        (0, 0, B_MIN - 1, "liquidity_below_min"),
        (0, 0, B_MAX + 1, "liquidity_above_max"),
        (Q_MAX + 1, 0, B_MIN, "quantity_above_max"),
        (0, Q_MAX + 1, B_MIN, "quantity_above_max"),
        (-1, 0, B_MIN, "quantity_negative"),
        (0, -1, B_MIN, "quantity_negative"),
    ]:
        out.append(fields(q_yes=q_yes, q_no=q_no, b=b, error=error))
    return out


def price_cases(rng: SplitMix64) -> list[dict]:
    out = []
    for q_yes, q_no, b in lmsr_inputs(rng, GROUP_SIZES["price"]):
        out.append({**fields(q_yes=q_yes, q_no=q_no, b=b), **price(q_yes, q_no, b)})
    return out


def cost_to_buy_cases(rng: SplitMix64) -> list[dict]:
    out = []
    edges = [
        (0, 0, B_MIN, "yes", 1), (0, 0, B_MIN, "no", 1),
        (0, 0, B_MAX, "yes", Q_MAX), (0, 0, B_MIN, "no", Q_MAX),
        (Q_MAX - 1, 0, B_MIN, "yes", 1), (0, Q_MAX - 1, B_MAX, "no", 1),
        (Q_MAX // 2, Q_MAX // 2, B_MAX, "yes", Q_MAX // 2),
    ]
    for case in edges:
        f, inexact = cost_to_buy(*case)
        out.append(fields(q_yes=case[0], q_no=case[1], b=case[2], outcome=case[3], d=case[4],
                          floor=f, inexact=inexact, base_units=ceil_div(f + (1 if inexact else 0), BASE_UNIT_SCALE)))
    for q_yes, q_no, b in lmsr_inputs(rng, GROUP_SIZES["cost_to_buy"] - len(edges) - 3):
        outcome = "yes" if rng.below(2) else "no"
        room = Q_MAX - (q_yes if outcome == "yes" else q_no)
        if room == 0:
            outcome = "no" if outcome == "yes" else "yes"
            room = Q_MAX - (q_yes if outcome == "yes" else q_no)
        if room == 0:
            continue
        d = rng.choice([1, room, rng.log_uniform(1, room)])
        f, inexact = cost_to_buy(q_yes, q_no, b, outcome, d)
        # Base units round up: the vault never undercharges (I15).
        out.append(fields(q_yes=q_yes, q_no=q_no, b=b, outcome=outcome, d=d,
                          floor=f, inexact=inexact, base_units=ceil_div(f + (1 if inexact else 0), BASE_UNIT_SCALE)))
    for q_yes, q_no, outcome, d, error in [
        (0, 0, "yes", 0, "quantity_not_positive"),
        (Q_MAX, 0, "yes", 1, "quantity_above_max"),
        (0, 0, "no", Q_MAX + 1, "quantity_above_max"),
    ]:
        out.append(fields(q_yes=q_yes, q_no=q_no, b=B_MIN, outcome=outcome, d=d, error=error))
    return out


def subsidy_cases(rng: SplitMix64) -> list[dict]:
    bs = [B_MIN, B_MAX, B_MIN + 1, B_MAX - 1, 10 * WAD, 1234567 * WAD]
    while len(bs) < GROUP_SIZES["subsidy"]:
        bs.append(rng.log_uniform(B_MIN, B_MAX))
    out = []
    for b in bs:
        f, inexact = real(lambda b=b: mpf(b) * mpmath.log(2), int_digits_hint=digits(b))
        ceil_wad = f + (1 if inexact else 0)
        # subsidy = ceil(b * ln 2) in base units: round up, the creator funds the worst case (I3).
        out.append(fields(b=b, cost_at_origin_floor=f, inexact=inexact,
                          base_units=ceil_div(ceil_wad, BASE_UNIT_SCALE)))
    return out


def conversion_cases() -> list[dict]:
    """WAD to base units at the boundary: costs up, shares down (I15). Exact integer math."""
    s = BASE_UNIT_SCALE
    values = [0, 1, s - 1, s, s + 1, 2 * s - 1, Q_MAX - 1, Q_MAX, Q_MAX + 1,
              U128_MAX, U128_MAX * s - 1, U128_MAX * s, U128_MAX * s + 1, (U128_MAX + 1) * s]
    out = []
    for kind in ("cost", "shares"):
        for v in values:
            rounded = ceil_div(v, s) if kind == "cost" else v // s
            if rounded > U128_MAX:
                out.append(fields(kind=kind, wad=v, error="above_u128"))
            else:
                out.append(fields(kind=kind, wad=v, base_units=rounded))
        out.append(fields(kind=kind, wad=-1, error="negative"))
    return out


# ------------------------------------------------------------------ output


def generator_blob() -> str:
    """Git blob hash of this file: identifies the generator source and survives squash merges."""
    here = Path(__file__).resolve()
    return subprocess.run(["git", "hash-object", str(here)], check=True, capture_output=True,
                          text=True, cwd=here.parent).stdout.strip()


def json_value(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, int):
        return str(v)
    return '"' + str(v).replace("\\", "\\\\").replace('"', '\\"') + '"'


def json_object(d: dict) -> str:
    return "{" + ", ".join(f'"{k}": {json_value(v)}' for k, v in d.items()) + "}"


def write(path: Path, doc: dict, groups: dict) -> None:
    lines = ["{"]
    for key in ("schema", "provenance", "constants"):
        value = doc[key]
        rendered = json_object(value) if isinstance(value, dict) else json_value(value)
        lines.append(f'  "{key}": {rendered},')
    names = list(groups)
    for gi, name in enumerate(names):
        lines.append(f'  "{name}": [')
        cases = groups[name]
        for ci, case in enumerate(cases):
            lines.append("    " + json_object(case) + ("," if ci < len(cases) - 1 else ""))
        lines.append("  ]" + ("," if gi < len(names) - 1 else ""))
    lines.append("}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", required=True, help="path to write the vector JSON")
    args = parser.parse_args()

    rng = SplitMix64(SEED)
    groups = {
        "exp_wad": exp_cases(rng),
        "ln_wad": ln_cases(rng),
        "cost": cost_cases(rng),
        "price": price_cases(rng),
        "cost_to_buy": cost_to_buy_cases(rng),
        "subsidy": subsidy_cases(rng),
        "conversion": conversion_cases(),
    }
    total = sum(len(v) for v in groups.values())
    if total < 5000:
        raise SystemExit(f"refusing to write only {total} cases; PHASES.md requires at least 5,000")

    doc = {
        "schema": SCHEMA,
        "provenance": {
            "generator": "tools/reference/lmsr_ref.py",
            "generator_blob": generator_blob(),
            "mpmath": mpmath.__version__,
            "python": f"{sys.version_info.major}.{sys.version_info.minor}",
            "precision_digits": PRECISION_DIGITS,
            "guard_digits_below_wei": GUARD_DIGITS,
            "stability_check_extra_digits": EXTRA_DIGITS,
            "seed": SEED,
            "prng": "splitmix64",
            "solady_commit": "9fe23ffdcd395c4169396064226ed27206b222c3",
        },
        "constants": {
            "wad": WAD, "base_unit_scale": BASE_UNIT_SCALE, "b_min": B_MIN, "b_max": B_MAX,
            "q_max": Q_MAX, "u128_max": U128_MAX,
            "exp_zero_at": EXP_ZERO_AT, "exp_overflow_at": EXP_OVERFLOW_AT,
        },
    }
    write(Path(args.out), doc, groups)
    print(f"lmsr_ref: wrote {total} cases to {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
