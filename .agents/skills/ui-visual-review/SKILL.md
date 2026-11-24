---
name: ui-visual-review
description: "Trigger: UI visual review, igualar UI, referencia visual, screenshot diff, iterar UI, fixtures/ui. Iterate Camcraft UI against canonical PNG references."
license: Apache-2.0
metadata:
  author: "th3g3ntl3man"
  version: "2.0"
---

## Activation Contract

Use when changing Camcraft UI visuals against reference captures in `fixtures/ui/`, adding a visual review scene, or diagnosing a UI diff.

## Hard Rules

- `fixtures/ui/*.png` are canonical references. Never overwrite them automatically; only copy a reviewed actual by hand when the change is intentional.
- Compare real renders, not HTML artboards. Artboards in `design/` are design intent; `fixtures/ui/manifest.json` maps scene coverage and gaps.
- Keep scene setup deterministic: fixed route/state, fixed data, no time-dependent data.
- Do not claim visual parity from code changes alone; capture and report diff evidence.

## Capture Paths

| Path | When | Notes |
|------|------|-------|
| Sway headless window capture | Linux/Wayland dev, background | Adapt `scripts/ui-review-sway.sh` to launch the Tauri app/window; captures with `grim`, compares PNGs in `target/ui-review-diffs/` |
| Playwright/webview screenshot | CI or deterministic browser-level capture | Preferred when webview route can be rendered without full desktop shell |
| Manual capture | Harness not wired yet | Capture at reference size, save to `target/ui-review-diffs/<id>.actual.png`, compare with same diff policy |

## Decision Gates

| Need | Action |
|------|--------|
| Iterate on existing scene | Capture scene, inspect `target/ui-review-diffs/<id>.{actual,diff}.png`, change smallest UI code, rerun |
| Add a new reference scene | Add PNG to `fixtures/ui/`, add scene entry to `fixtures/ui/manifest.json`, wire deterministic scene setup |
| Accept intentional visual change | Review diff, then `cp target/ui-review-diffs/<id>.actual.png fixtures/ui/<id>.png` |
| Diff is render-noise heavy | Check font/render path, viewport size, theme, and scale before changing UI |

## Execution Steps

1. Read `fixtures/ui/manifest.json` for scene id, reference, artboard, covers, and gaps.
2. Capture the scene at the reference image size with deterministic state.
3. Compare against `fixtures/ui/<id>.png`; inspect actual and diff in `target/ui-review-diffs/`.
4. Change the smallest UI code that explains the diff; rerun affected scene.
5. When a scene matches, report diff percent and evidence. If updating reference intentionally, copy by hand and review before commit.

## Output Contract

Report: scenes run, pass/diff status, diff percent, actual/diff paths, reference touched (only if manually copied), and remaining visual gaps.

## References

- `fixtures/ui/manifest.json` — canonical scene manifest and update policy.
- `scripts/ui-review-sway.sh` — legacy sway headless capture script; adapt for Tauri window launch.
- `design/` — visual intent/artboards.
