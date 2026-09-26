# ADR-0014: leniency only where bytes never hash and never enter state

- **Status:** accepted (Jay, Q1 on #66)
- **Date:** 2026-09-26
- **Phase:** 2
- **Invariants touched:** I7 (a voucher is accepted only if valid), I12 (a receipt wins a dispute)

## Context

The x402 envelopes are JSON, and JSON is extensible. `docs/spec-notes.md` §3 lists optional fields
next to required ones, so a newer client may legitimately send keys we have never seen. Two
policies are available for every object we parse: refuse an unknown key, or ignore it.

Refusing everywhere makes a newer official SDK fail against us until we ship. Ignoring everywhere
means a field we do not understand can arrive beside a voucher without comment. Neither answer is
right for every object, and picking per object by taste is how a rule rots.

## Decision

> **Leniency is allowed exactly where the bytes never enter a hash preimage and never enter state.
> `extra` qualifies. Nothing else does.**

In practice:

| Object | Policy | Why |
|---|---|---|
| `PaymentPayload`, `Payload` | **deny unknown fields** | its keys carry money |
| `channelConfig` | **deny unknown fields** | it is the EIP-712 struct whose hash **is** the `channelId` |
| `voucher` | **deny unknown fields** | it is signed |
| `PaymentRequirements`, `PaymentRequired` | **deny unknown fields** | what we advertise, and what a client echoes back |
| `extra`, `channelState`, `voucherState` | **ignore unknown fields** | never hashed, and never read into our state |

**`channelConfig` is the case that matters most**, and it is the one a nesting-based rule would get
wrong. An unknown field there means the client signed a **different type string** than the one we
hash. Ignoring it gives us a `channelId` over the seven fields we know while the client signed
eight, and the mismatch then surfaces as `SIG_WRONG_SIGNER`: an error that sends them to look at
their key when the problem is their payload. Refusing it names the actual problem.

The same reasoning applies to anything added later: if the bytes reach a `keccak256` or reach the
ledger, they are strict. That is why the rule is stated about hashing rather than about depth.

## Consequences

A client that sends a field we have not implemented gets a 4xx naming the field rather than a
silent success. That is a real support cost, and it is the cheaper failure: the alternative is a
signature that verifies over a different struct than the one the client signed.

`extra` is where the scheme itself puts extensions, and it is the part the spec calls untrusted
(`docs/spec-notes.md` §3: the client "MUST NOT copy `extra.channelState`" into its own state). We
never read it into state either, so leniency there costs nothing and buys forward compatibility
with the official SDK.

When a newer SDK does add a top-level field, the fix is one line plus a vector, and CI tells us
which field. Nothing has to be guessed.

## How this is enforced

`#[serde(deny_unknown_fields)]` on every strict type, and
`engine/crates/protocol/tests/envelope.rs` plants an unknown field in each: refused at the top
level, inside `payload`, inside `channelConfig` and inside `voucher`, and accepted inside `extra`.
