# Nybble — Cleanup backlog

Sequenced **low-risk → high-risk** so each phase leaves the tree green and
gives the next phase a regression net. One logical change per commit (atomic).
Gate after every commit: `cargo fmt && cargo clippy && cargo test`.

Baseline at time of writing: `cargo test` 63 green; `cargo clippy` **13 warnings**
(12 in `nybble-core`, 1 in `nybble-gui`).

---

## Phase 1 — Quick wins (safe, no behavior change)

Goal: a clean `cargo clippy`, so it can act as a regression net for later phases.

- [x] **1a. Fix mechanical clippy lints in core.** — `Fix mechanical clippy lints in nybble-core`
  - `value.rs`, `ops.rs:186` — manual `(x + n - 1) / n` → `div_ceil`.
  - `value.rs:150` — manual modulo check → `is_multiple_of`.
  - `expr.rs`, `float.rs` — `loop { let Some.. else break }` → `while let`.
- [x] **1b. Fix the gui collapsible-if.** `app.rs` — collapsed the nested `if`. — `Collapse nested if in update-check branch`
- [x] **1c. Delete comment rot.** — `Remove stale doc comments in app.rs`
  - Stale `///` lines on `copy_icon_button` (deleted "accent stripe" helper).
  - `field_literal` doc opened with a line describing a different function.

**Exit:** ✅ Clippy down to **6 warnings**, all `should_implement_trait` on `Value`
(`not/add/sub/mul/neg/shl`) — these are Phase 2a, not suppressible noise.

---

## Phase 2 — API hygiene (improves design, lint-driven)

- [x] **2a. Implement `std::ops` traits on `Value` instead of inherent `add/sub/mul/neg/not/shl`.**
  - `ops.rs:45-113` — the `confusing_method_name` lints (`not/add/sub/mul/neg/shl`) are
    correct: for a numeric type these *should* be operator impls. Convert where the
    signature matches std (`Add/Sub/Mul/Neg/Not`); keep sign/width-aware ops
    (`div`, `rem`, `shr`) as inherent methods.
  - Update call sites in `expr.rs:283` (`apply_infix`) accordingly.
  - This removes ~6 warnings *and* lets expressions read naturally.
  - Commit: `Implement std::ops traits on Value`

**Exit:** core API uses operators; tests still green.

---

## Phase 3 — Move misplaced logic to where the docs say it lives

CLAUDE.md core rule: "all numeric logic lives in `nybble-core`" / "Never put
numeric decisions in the GUI." These violate it.

- [x] **3a. Move `parse_base` + `strip_radix_prefix` into core.**
  - From `app.rs:2567-2610` → `crates/core/src/parse.rs` (next to `parse_literal`,
    which already does prefix handling — dedupe against it).
  - Re-export from `lib.rs`; update the two callers (`on_field_edit`, `on_field_edit_float`).
  - Add core tests for the radix/sign/whitespace cases.
  - Commit: `Move base-field parsing into nybble-core`
- [x] **3b. Add a `buffer_mut(field) -> &mut String` accessor on `App`.**
  - Kills the 4 hand-written `match field { Hex => &mut self.hex, … }` blocks
    (`app.rs:579`, `617`, `1129`, `1182`) and their `unreachable!()` arms.
  - Commit: `Add App::buffer_mut to dedupe field/buffer dispatch`
- [x] **3c. Extract the drawn icon buttons into `widgets/`.**
  - `copy/send/settings/close/triangle/theme` icons + `draw_*` glyphs (`app.rs:2000-2305`)
    → `crates/gui/src/widgets/icons.rs`, re-export via `widgets/mod.rs`
    (the documented convention; `bitgrid.rs` already follows it).
  - Commit: `Extract drawn icon buttons into widgets::icons`

**Exit:** `app.rs` no longer owns parsing or reusable widgets.

---

## Phase 4 — High-risk refactors (do LAST, behind tests)

⚠️ The GUI has **zero automated tests**. Do 4a before 4b/4c or regressions will
slip in silently.

- [x] **4a. Characterization tests on the `App` state machine.**
  - Cover `set_number_mode` (int↔float seeding, `app.rs:539`), `refresh`/`refresh(skip)`
    (`app.rs:456`), `recall` (`app.rs:689`), `on_field_edit` round-trips. These need no
    egui context — they're pure state transitions on `App` fields.
  - Commit: `Add characterization tests for App state transitions`
- [x] **4b. Split `app.rs` (2611 lines) along section boundaries.**
  - e.g. `app/state.rs` (struct + new/save), `app/sections.rs` (panel render fns),
    `app/layout.rs` (column balancing, resize detection). Incremental, one move per commit.
  - `app/sections.rs`: all UI panel rendering methods + drawing helpers (~1350 lines)
  - `app/layout.rs`: `panel_visible`, `visible_panels`, `balance_columns`
  - `app/mod.rs`: types, struct, state machine, eframe impl, tests (~680 lines)
- [ ] **4c. (Optional) Dedupe the `expr.rs` / `float.rs` tokenizers.**
  - ~300 lines of near-parallel tokenizer + Pratt loop. A shared generic tokenizer
    is possible but couples two intentionally-separate evaluators — only if the
    maintenance cost is actually biting.

**Exit:** decide per-item; 4c may be declined.

---

## Notes
- Silent exponent truncation `exp.raw() as u32` (`ops.rs:176`) is *known* (test at
  `expr.rs:510`). Out of scope unless it bites — listed for visibility.
- No CI configured, so the per-commit gate above is the only safety net. Keep it green.
</content>
</invoke>
