# RCA — Phase 1–3 Completion Audit

**Date:** 2026-11-16
**Repo:** `~/projects/rca`
**Source basis:** `docs/ROADMAP.md`, `README.md`, `CHANGELOG.md`, `src/` grep verification
**Scope note:** Docs + source-level grep audit. Not a full runtime/test-suite audit.

---

## Summary

| Category | Count |
|---|---|
| Phase 1 features | 8 (+1 milestone) |
| Phase 2 features | 4 bullets → 10 items + 1 confirmed gap |
| Phase 3 features | 7 (+1 milestone) + native backlog |
| **Confirmed gaps (P1)** | **1** |
| **Partial / temp (P2)** | **2** |
| **Docs inconsistencies (P3)** | **5** |
| Deferred (P4) | 8 |

---

## Phase 1 — Language Specification and Compiler Foundation

Roadmap: `docs/ROADMAP.md` L421–434

| # | Feature | Source evidence | Status |
|---|---|---|---|
| 1.1 | Finalize syntax specification | Not evidenced as finalized in changelog | ⚠️ UNCLEAR |
| 1.2 | Define token types | `src/compiler/lexer/token.rs` | ✅ |
| 1.3 | Implement lexer | `src/compiler/lexer/{mod,scanner}.rs` | ✅ |
| 1.4 | Implement parser | `src/compiler/parser/mod.rs` | ✅ |
| 1.5 | Design AST | `src/compiler/ast/mod.rs` (`ExprKind`, `StmtKind`) | ✅ |
| 1.6 | Source location tracking | `Span` in `lexer/token.rs`, `scanner.rs` | ✅ |
| 1.7 | Compiler diagnostics | `src/compiler/diagnostics/mod.rs` | ✅ |
| 1.8 | Unit tests | `tests/` (40+ files) | ✅ |
| M1 | Milestone: parses valid, reports syntax errors | Parser tests | ✅ |

---

## Phase 2 — Core Language

Roadmap: `docs/ROADMAP.md` L435–443

| # | Feature | Source evidence | Status |
|---|---|---|---|
| 2.1a | Variables | checker, parser | ✅ |
| 2.1b | Data types (`List[T]`, `Map[K,V]`, `Array[T,N]`) | `types/mod.rs`, `checker.rs:131` | ✅ |
| 2.1c | Type inference | `checker.rs` `fn infer` | ✅ |
| 2.2 | Static type checking | `src/compiler/semantic/checker.rs` | ✅ |
| 2.3a | Functions | parser, checker, IR | ✅ |
| 2.3b | Conditionals | parser, checker | ✅ |
| 2.3c | Loops | parser, checker | ✅ |
| 2.3d | Structures | `pt.Point(x=1).x` (CHANGELOG L13) | ✅ |
| 2.4a | Scope management | `src/compiler/semantic/scope.rs` | ✅ |
| 2.4b | Modules | `ModuleExports`, `apply_imports`, `collect_exports` (CHANGELOG L10, L11) | ✅ |
| **2.5** | **Module-qualified type in function signature** (`fn f(p: pt.Point)`) | **ZERO match in `src/`**; CHANGELOG L13 "Known gap... still unresolved" | ❌ **GAP** |
| M2 | Milestone: meaningful subset of own language | Met except 2.5 | ⚠️ PENDING 2.5 |

---

## Phase 3 — IR and Execution

Roadmap: `docs/ROADMAP.md` L444–454

| # | Feature | Source evidence | Status |
|---|---|---|---|
| 3.1 | HIR | `src/compiler/hir/mod.rs` | ✅ (with shim — see 3.8) |
| 3.2 | RCA IR | `src/compiler/ir/mod.rs` | ✅ |
| 3.3 | IR validation | `src/compiler/ir/validate.rs` | ✅ |
| 3.4 | Initial execution engine | `runtime/vm/mod.rs`, `runtime/execution/mod.rs` | ✅ |
| 3.5 | Native compilation backend | `src/compiler/codegen/mod.rs`; README "growing subset" | ⚠️ **PARTIAL** |
| 3.6 | Runtime integration | `runtime/` | ✅ |
| 3.7 | Optimization passes | `src/compiler/optimizer/mod.rs` | ✅ |
| **3.8** | **HIR→AST shim removal** | `lower.rs:80, 321, 337` "temporary shim" | ⚠️ **TEMP** |
| M3 | Milestone: execute independently of Python | VM runs bytecode | ✅ |

### 3.5 — Native backend backlog (CHANGELOG `Planned:` L100–188)

Every `Planned:` line in this range is native-codegen scope. Grouped:

- [ ] Full IR-to-Cranelift translation + final executable linking (L100, L102)
- [ ] Arithmetic, strings, functions, control flow native (L104–L110)
- [ ] String concat, indexing, `len()`, functions (L112)
- [ ] `Call`/`Return`, `for` loops (L114–L118)
- [ ] Recursion, string/float params, default args (L120)
- [ ] Float↔Int, `Char`/`List`/`Map`/`Struct` native (L122)
- [ ] String params, String ordering, `Enum` native (L124)
- [ ] Struct/Enum/List param types, default/named args (L126)
- [ ] `str()`, string↔numeric, Bool↔numeric (L128)
- [ ] Struct as param/return, nested structs, display/equality, COW (L130)
- [ ] Stack-balance audit (L132)
- [ ] Float `nan`/`inf` formatting parity (L134)
- [ ] Source-span errors, `try`/`catch` in native (L136–L138)
- [ ] `INT_MIN` literal, `try`/`catch` across fns (L138)
- [ ] Unary `Neg` in native (L140)
- [ ] Float shortest-rep formatting (L142)
- [ ] Sized `rca_globals`, out-of-range guard (L144)
- [ ] Trailing-space + struct display in `print` (L146)
- [ ] Return-type inference for unannotated fns (L148)
- [ ] `Not` in native (L150)
- [ ] Short-circuit `and`/`or` in native (L152)
- [ ] `print(list)`, list indexing/`push`, shared layout (L154–L158)
- [ ] `StoreIndex` (`xs[i] = v`), list `pop`/`remove` (L160–L162)
- [ ] Tuple/string iteration in `ForNext` (L164)
- [ ] Tuple indexing, destructuring (L166–L168)
- [ ] **Enum payloads, `match` as expression, match guards — checker-level done, native needed (L170)**
- [ ] `map_set`/`map_get`/`map_has`, map display, `free` (L172–L174)
- [ ] Map value-type tracking (L176–L178)
- [ ] Exact saturation/domain-error parity for `pow` (L180)
- [ ] Source-span errors, `try`/`catch` across fn boundaries (L182)
- [ ] Incref/decref, `free` for heap types, leak tests (L184–L188)

### 3.8 — HIR shim detail

- `src/compiler/ir/lower.rs:80` — `lower_hir_to_ast(&hstmts)`
- `src/compiler/ir/lower.rs:321` — "Step 4b-1: HIR statement → AST (temporary shim)"
- `src/compiler/ir/lower.rs:337` — "Step 4b-1: HIR expression → AST (temporary shim)"
- CHANGELOG L23 — "subsequent 4b-N steps will replace each shim with native `HStmtKind`/`HExprKind` handling"

---

## Changelog `Planned:` — Post-Phase 3 (but changelog mentions)

These are **not** Phase 1–3 roadmap requirements, but recorded because changelog tracks them:

| Line | Item | Category |
|---|---|---|
| 81 | `fs.read/write/exists` module-qualified | stdlib |
| 83 | `time.now/sleep` module-qualified | stdlib |
| 85 | `math.abs/pow` module-qualified | stdlib |
| 87 | `string.trim` module-qualified | stdlib |
| 90 | Map literal syntax `{"k": v}` | language |
| 92 | Native JSON + Map literal | language / stdlib |
| 94 | UDP sockets, HTTP wrappers | stdlib |
| 96 | HTTP POST, HTTPS | stdlib |
| 98 | Additional hashes (SHA-512, MD5), HMAC | stdlib |

**⚠️ Stale check:** CHANGELOG L52–55 says HMAC, SHA-512, HTTP POST, UDP are **already implemented** — verify whether L94–L98 should be removed.

---

## Docs inconsistencies

| # | Item | README | CHANGELOG | Source | Verdict |
|---|---|---|---|---|---|
| D1 | `pub const` | L117: "not implemented" | L16: "supported from #174" | Parser tests + `StmtKind::Let.public` | ❌ README wrong |
| D2 | `Array[T, N]` | Not mentioned | Fixed-size arrays added | `parser/mod.rs:339`, `types/mod.rs:70`, `checker.rs:131` | ⚠️ README update |
| D3 | Match expression | "not implemented" | L170: "checker-level feature" | `StmtKind::Match` exists; `ExprKind::Match` not found | ⚠️ Statement exists, expression pending |
| D4 | "Phase 2 milestone met" | — | L17 says met | L13 lists known gap open | ❌ Contradiction |
| D5 | CHANGELOG L94–98 | — | "Planned" | L52–55 implemented | ⚠️ Stale |

---

## Priority list

### P1 — Blockers (Phase 2 milestone tihkim theih lohna)

| # | Item | Fix |
|---|---|---|
| P1.1 | Module-qualified type in fn signature (`fn f(p: pt.Point)`) | Resolve `pt.Point` param type via `ModuleExports`; add `tests/struct_import.rs` case; remove "Known gap" from CHANGELOG L13 |

### P2 — Closeout (partial / temp)

| # | Item | Fix |
|---|---|---|
| P2.1 | HIR→AST shim (`lower.rs:321,337`) | Replace shims with native `HStmtKind`/`HExprKind` handling |
| P2.2 | Native backend completeness | Work through CHANGELOG `Planned:` L100–L188 |
| P2.3 | Module system file separation | `ModuleExports`/`apply_imports` only in `checker.rs` — consider `src/compiler/modules/` extraction |

### P3 — Docs fix

| # | Item | Fix |
|---|---|---|
| P3.1 | `pub const` in README | Remove from "Not implemented yet" |
| P3.2 | `Array[T, N]` in README | Add to working features |
| P3.3 | Match expression README | Clarify: statement exists, expression pending native |
| P3.4 | CHANGELOG L17 | Soften "milestone met" while L13 gap open |
| P3.5 | CHANGELOG L94–L98 | Verify implemented; remove stale Planned entries |

### P4 — Deferred (not in Phase 1–3 roadmap, changelog mentions)

| # | Item | Notes |
|---|---|---|
| P4.1 | Generics | Not in roadmap Phase 1–3 |
| P4.2 | Enum payloads end-to-end | Checker-level done, native pending (3.5) |
| P4.3 | Match expression end-to-end | Statement done, expression pending native |
| P4.4 | Stdlib module-qualified (`fs`, `time`, `math`, `string`) | CHANGELOG L81–87 |
| P4.5 | Map literal syntax | CHANGELOG L90 |
| P4.6 | HTTP/UDP/crypto extensions | CHANGELOG L94–98 (verify stale) |
| P4.7 | Borrowing and references | README |
| P4.8 | Autodiff `max`/`min`, tensor method syntax, GPU | README |

---

## Verification commands

```bash
cd ~/projects/rca

# Phase 1
ls src/compiler/lexer/ src/compiler/parser/ src/compiler/ast/ src/compiler/diagnostics/

# Phase 2 gap
grep -rn "pt.Point\|qualified_type\|QualifiedType" src/ || echo "GAP: no module-qualified type"

# Phase 3 native + shim
grep -n "growing subset" README.md
grep -n "temporary shim\|lower_hir_to_ast" src/compiler/ir/lower.rs

# Docs inconsistencies
grep -n "pub const" README.md CHANGELOG.md
grep -n "Phase 2 milestone met" CHANGELOG.md
grep -n "Array\[" src/compiler/parser/mod.rs src/compiler/types/mod.rs

# Changelog backlog
grep -n "Planned:" CHANGELOG.md
```

---

## Roadmap addendum — proposed

If you want to record open items in `docs/ROADMAP.md`, append after Phase 3:

```markdown
### Phase 2 — Open gaps

- [ ] Module-qualified type annotations in function signatures (`fn f(p: pt.Point)`).

### Phase 3 — Open backlog

- [ ] Full IR-to-Cranelift translation (see CHANGELOG "Planned" L100–L188).
- [ ] HIR→IR direct lowering (remove temporary HIR→AST shims in `ir/lower.rs`).
- [ ] Enum payloads, match expressions, match guards in native codegen.
```

---

## Sources

- `docs/ROADMAP.md` L421–455
- `README.md` L68–130
- `CHANGELOG.md` L10–23, L52–55, L81–188
- `src/compiler/semantic/checker.rs`
- `src/compiler/ir/lower.rs` L80, L321, L337
- `src/compiler/codegen/mod.rs`
- `src/compiler/parser/mod.rs:339, 506`
- `src/compiler/types/mod.rs:70`
