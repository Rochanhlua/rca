# RCA — TODO (Phase 1–3)

Phase 1: complete — not listed.

---

## Phase 2 — remaining

- [ ] **Module-qualified type in function signature** — `fn f(p: pt.Point)`
      CHANGELOG L13 "Known gap... still unresolved"
      `grep -rn "pt.Point" src/` → zero match

---

## Phase 3 — remaining

- [ ] **Native compilation backend** — complete
      README: "growing subset"; CHANGELOG "Planned" L100–L188 (~45 items)

- [ ] **HIR→IR direct lowering** — remove temporary HIR→AST shim
      `src/compiler/ir/lower.rs:321,337` "temporary shim"
      Replace with native `HStmtKind`/`HExprKind` handling

- [ ] **Enum payloads** — end-to-end (native) — CHANGELOG L170
- [ ] **Match as expression** — statement exists; expression pending native — CHANGELOG L170
- [ ] **Match guards** — CHANGELOG L170

---

## Changelog `Planned:` — stdlib / language (verified NOT implemented)

- [ ] `fs.read` / `fs.write` / `fs.exists` — module-qualified (`import fs`) — L81
- [ ] `time.now` / `time.sleep` — module-qualified (`import time`) — L83
- [ ] `math.abs` / `math.pow` — module-qualified (`import math`) — L85
- [ ] `string.trim` — module-qualified (`import string`) — L87
- [ ] Map literal syntax `{"k": v}` + direct iteration — L90
- [ ] Native JSON integration with Map literal — L92

---

## Docs cleanup (feature a ni lo)

- [ ] README: remove `pub const` from "Not implemented yet" (already implemented)
- [ ] README: add `Array[T, N]` to working features
- [ ] README: clarify `match` — statement exists, expression pending
- [ ] CHANGELOG L17: soften "Phase 2 milestone met" (L13 gap still open)
- [ ] CHANGELOG L94–L98: remove stale `Planned` entries
      (UDP L55, HTTP POST L54, SHA-512 L53, HMAC L52 already done)
