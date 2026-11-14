# RCA Crypto

Status: SHA-256, SHA-512, and HMAC-SHA256 are implemented as builtins and
as `import crypto` members. Uses the audited RustCrypto `sha2` and `hmac`
crates (see §4). No invented cryptography.

## 1. Functions

| Builtin | Module form | Returns |
|---|---|---|
| `sha256(s)` | `crypto.sha256(s)` | 64-char lowercase hex |
| `sha512(s)` | `crypto.sha512(s)` | 128-char lowercase hex |
| `hmac_sha256(k, m)` | `crypto.hmac_sha256(k, m)` | 64-char lowercase hex |

All arguments must be `String`. Non-String arguments are a compile-time
error.

## 2. Examples

    import crypto

    print(crypto.sha256("abc"))
    # ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad

    print(crypto.sha512("abc"))
    # ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a
    # 2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f

    print(crypto.hmac_sha256("key", "The quick brown fox jumps over the lazy dog"))
    # f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8

(SHA-256 "abc" and both SHA-512 / HMAC vectors are FIPS 180-4 / RFC 4231
known-answer values; the tests in `tests/crypto*.rs` pin them.)

## 3. Constraints and limits
- **No key derivation** (PBKDF2, scrypt, argon2) yet.
- **No HMAC-SHA512** yet (trivial follow-up; same crate).
- **No MD5** — legacy only, deliberately not added.
- **No streaming API** — arguments are complete strings, not chunks.
- **No `pub const`-friendly form** — call as a function.
- **No native codegen** — runs on the VM (Cranelift backend falls through
  to "not yet implemented"; use `rcac run`).

## 4. Cryptographic correctness policy
Per blueprint §9: algorithms and implementations must be audited, not
invented. We use:
- `sha2 = "0.10"` (RustCrypto) — SHA-256, SHA-512
- `hmac = "0.12"` (RustCrypto) — HMAC over `sha2`

Both crates are audited by the RustCrypto team. Adding a new primitive
(e.g. HMAC-SHA512, PBKDF2) means using a RustCrypto crate that implements
it — not re-implementing on top of `sha2` unless the standard is small
and well-reviewed (RFC 2104-style).

## 5. Tests
- `tests/crypto.rs` — SHA-256 known-answer.
- `tests/crypto_sha512.rs` — SHA-512 empty and "abc" vectors, type error.
- `tests/crypto_hmac.rs` — RFC 4231 case 2, key+"quick brown fox",
  empty/empty (deterministic 64-hex), type error, arity.
