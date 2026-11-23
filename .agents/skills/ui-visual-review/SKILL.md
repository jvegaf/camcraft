---
name: ui-visual-review
description: "Trigger: UI visual review, igualar UI, referencia visual, screenshot diff, iterar UI, fixtures/ui. Iterate cam-ui egui screens against canonical PNG references."
license: Apache-2.0
metadata:
  author: "th3g3ntl3man"
  version: "1.0"
---

## Activation Contract

Use when changing cam-ui visuals against reference captures in `fixtures/ui/`, adding a visual review scene, or diagnosing a UI diff.

## Hard Rules

- `fixtures/ui/*.png` are canonical references. Never overwrite them automatically; only copy a reviewed actual by hand when the change is intentional.
- Compare real egui renders, not HTML artboards. Artboards in `design/` are design intent; `fixtures/ui/manifest.json` maps scene coverage and gaps.
- Render at the reference image size (`image_dimensions`), `pixels_per_point=1.0`, same harness warmup as `crates/cam-ui/tests/ui_review.rs`.
- Keep scene setup deterministic: fixed shell state, fixed document/recents, no time-dependent data.
- Do not claim visual parity from code changes alone; run `just ui-review` and report diff evidence.

## Decision Gates

| Need | Action |
|------|--------|
| Iterate on existing scene | Run `just ui-review-sway` (background, no toca Hyprland) o `just ui-review` (egui_kittest); inspeccionar `target/ui-review-diffs/<id>.{actual,diff}.png`, cambiar UI, rerun |
| Add a new reference scene | Add PNG to `fixtures/ui/`, add scene entry to `fixtures/ui/manifest.json`, add `#[ignore]` test in `crates/cam-ui/tests/ui_review.rs` |
| Accept intentional visual change | Review diff, then `cp target/ui-review-diffs/<id>.actual.png fixtures/ui/<id>.png` |
| UI differs but render is blank/transparent | Treat as environment/render issue; verify wgpu backend before changing UI |

## Execution Steps

1. Read `fixtures/ui/manifest.json` for scene id, reference, artboard, covers, and gaps.
2. Run `just ui-review-sway` para captura en background con sway headless (recomendado en esta máquina), o `just ui-review` para egui_kittest/wgpu.
3. For each failing scene, open `target/ui-review-diffs/<id>.actual.png` and `<id>.diff.png`; compare against `fixtures/ui/<id>.png`.
4. Change the smallest cam-ui code that explains the diff; rerun only affected scene with `cargo test -p cam-ui --test ui_review <scene> -- --ignored --nocapture` while iterating.
5. When a scene matches, confirm output reports exact match or within tolerance (0.50% pixels, bounded channel diff).
6. If adding a scene, wire shell state in `ui_review.rs`: create `Shell`/`AppState`, set deterministic data, call `shell.show_in(app, ui)`, render, `compare("<id>", &img)`.

## Output Contract

Report: scenes run, pass/diff status, diff percent, actual/diff paths, reference touched (only if manually copied), and remaining visual gaps.

## References

- `fixtures/ui/manifest.json` — canonical scene manifest and update policy.
- `crates/cam-ui/tests/ui_review.rs` — offscreen egui_kittest/wgpu render and diff harness.
- `scripts/ui-review-sway.sh` — sway headless capture + diff harness (background).
- `justfile` — `ui-review`, `ui-review-sway`, and `ui-review-update` recipes.
