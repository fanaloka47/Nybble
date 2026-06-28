# Nybble — Cleanup backlog

Sequenced **low-risk → high-risk** so each phase leaves the tree green and
gives the next phase a regression net. One logical change per commit (atomic).
Gate after every commit: `cargo fmt && cargo clippy && cargo test`.

Baseline at time of writing: `cargo test` 63 green; `cargo clippy` **13 warnings**
(12 in `nybble-core`, 1 in `nybble-gui`).

---

## Phase 1 — Quick wins (safe, no behavior change)

Goal: a clean `cargo clippy`, so it can act as a regression net for later phases.

- [ ] **1a. Fix mechanical clippy lints in core.**
  - `value.rs:115`, `value.rs:129`, `ops.rs:186` — replace manual `(x + n - 1) / n` with `div_ceil`.
  - `value.rs:150` — replace manual modulo check with `is_multiple_of`.
  - `expr.rs:241`, `float.rs:190` — rewrite the `loop { let Some.. else break }` as `while let`.
  - Commit: `Fix mechanical clippy lints in nybble-core`
- [ ] **1b. Fix the gui collapsible-if.** `app.rs:1896` — collapse the nested `if`.
  - Commit: `Collapse nested if in update-check branch`
- [ ] **1c. Delete comment rot.**
  - `app.rs:1995-1999` — three stale `///` lines on `copy_icon_button` describing a deleted "accent stripe" helper.
  - `app.rs:560-563` — `field_literal` doc opens "Parse the buffer for `field`…", describing a different function.
  - Commit: `Remove stale doc comments in app.rs`

**Exit:** `cargo clippy` clean (0 warnings).

---

## Phase 2 — API hygiene (improves design, lint-driven)

- [ ] **2a. Implement `std::ops` traits on `Value` instead of inherent `add/sub/mul/neg/not/shl`.**
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

- [ ] **3a. Move `parse_base` + `strip_radix_prefix` into core.**
  - From `app.rs:2567-2610` → `crates/core/src/parse.rs` (next to `parse_literal`,
    which already does prefix handling — dedupe against it).
  - Re-export from `lib.rs`; update the two callers (`on_field_edit`, `on_field_edit_float`).
  - Add core tests for the radix/sign/whitespace cases.
  - Commit: `Move base-field parsing into nybble-core`
- [ ] **3b. Add a `buffer_mut(field) -> &mut String` accessor on `App`.**
  - Kills the 4 hand-written `match field { Hex => &mut self.hex, … }` blocks
    (`app.rs:579`, `617`, `1129`, `1182`) and their `unreachable!()` arms.
  - Commit: `Add App::buffer_mut to dedupe field/buffer dispatch`
- [ ] **3c. Extract the drawn icon buttons into `widgets/`.**
  - `copy/send/settings/close/triangle/theme` icons + `draw_*` glyphs (`app.rs:2000-2305`)
    → `crates/gui/src/widgets/icons.rs`, re-export via `widgets/mod.rs`
    (the documented convention; `bitgrid.rs` already follows it).
  - Commit: `Extract drawn icon buttons into widgets::icons`

**Exit:** `app.rs` no longer owns parsing or reusable widgets.

---

## Phase 4 — High-risk refactors (do LAST, behind tests)

⚠️ The GUI has **zero automated tests**. Do 4a before 4b/4c or regressions will
slip in silently.

- [ ] **4a. Characterization tests on the `App` state machine.**
  - Cover `set_number_mode` (int↔float seeding, `app.rs:539`), `refresh`/`refresh(skip)`
    (`app.rs:456`), `recall` (`app.rs:689`), `on_field_edit` round-trips. These need no
    egui context — they're pure state transitions on `App` fields.
  - Commit: `Add characterization tests for App state transitions`
- [ ] **4b. Split `app.rs` (2611 lines) along section boundaries.**
  - e.g. `app/state.rs` (struct + new/save), `app/sections.rs` (panel render fns),
    `app/layout.rs` (column balancing, resize detection). Incremental, one move per commit.
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
