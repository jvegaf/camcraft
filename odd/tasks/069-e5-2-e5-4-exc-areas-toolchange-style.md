# 069 — E5.2 exc_areas + travel routing; E5.4 toolchange_style

## Surfaces

- `crates/cam-model/src`
- `crates/cam-move/src`
- `crates/cam-ops/src`
- `crates/cam-sim/src`
- `crates/cam-cli/src`
- `crates/cam-cli/tests`
- `crates/cam-post/src`
- `crates/cam-post/posts`
- `crates/cam-post/tests`
- `docs/ROADMAP_CAMCRAFT.md`
- `odd/roadmap/index.md`
- `odd/tasks/069-e5-2-e5-4-exc-areas-toolchange-style.md`

## Specs

- **S1**: "ahora para la task 069 vamos con E5.2 y E5.4"
- **S2** (roadmap E5.2, `docs/ROADMAP_CAMCRAFT.md`): "| E5.2 | `exc_areas` en el modelo + ruteo de travel alrededor de exclusiones | **M** | Un travel que cruza un área prohibida se desvía; simulación sin colisión |"
- **S3** (roadmap E5.4, `docs/ROADMAP_CAMCRAFT.md`): "| E5.4 | `toolchange_style` como enum en el preprocesador, reemplazando el matching por nombre de FlatCAM (`CNCJobObject.py:947-962`) | **S** | Los dialectos con M6 especial emiten igual que el original; golden |"
- **S4** (user decision, 2026-11-21): "Para E5.2 (`exc_areas` en el modelo + ruteo de travel), ¿dónde lo ponemos y cómo lo modelamos? ... — Document.exc_areas (Recommended)" / "Vec<Region> en Document, default vacío, backcompat. Ruteo como transform de moves antes de emit/sim. Keepout separado."
- **S5** (user decision, 2026-11-21): "Para E5.4 (`toolchange_style` enum)... — Enum + directive + goldens (Recommended)" / "Agregar toolchange_style enum/directiva en PostProcessor, default compatible, comportamiento por estilo en emit, goldens M6 especial."

## Tasks

| ID | Spec | Route | Commit | Status |
| --- | --- | --- | --- | --- |
| T1 | S3,S5 | inline | 18af779 | done | Add `toolchange_style` enum/directive to cam-post, default compatible, emit by style, goldens for special M6 dialects |
| T2 | S2,S4 | inline | 3d4c012 | done | Add `exc_areas: Vec<Region>` to Document (serde default, backcompat) and model/validation plumbing |
| T3 | S2,S4 | inline | — | pending | Implement travel routing around exc_areas for Rapid/Retract moves and wire into G-code-producing paths before emit/sim |
| T4 | S2,S4 | inline | — | pending | Extend simulation/collision checks for travel across exclusion areas; add tests proving travel deviates and sim has no collision |
| T5 | S2,S3 | inline | — | pending | Update roadmap/docs status and task evidence; run focused checks |

## Log

- **L6** (assistant): T3/T4 added exc_travel routing (Rapid/Retract detours around exc_areas, per-segment tool clearance), wired into isolate/cutout/program/ncc CLI emit paths, extended cam-sim with apply_with_exc_areas collision checks, and added acceptance tests for deviation and no-collision.

- **L5** (assistant): T2 added Document.exc_areas: Vec<Region> with serde default/skip_serializing_if empty, Region serde derives, check_exc_areas validation, and document round-trip/default/reject tests. cam-model tests pass.

- **L4** (assistant): T1 implemented ToolchangeStyle enum (Standard/MarlinM6/RepetierPause), TOOLCHANGE_STYLE directive parsing, emit style dispatch, marlin/repetier posts declare style; added toolchange goldens for marlin/repetier. cam-post tests pass.

- **L1** (user request, verbatim): "ahora para la task 069 vamos con E5.2 y E5.4"
- **L2** (assistant): Explored E5.2/E5.4 surfaces via gentle-ai-explore. Key findings: no `exc_areas`/`toolchange_style` in crates; cam-move has Rapid/Retract; cam-sim only checks rapid below surface; cam-post has fixed TOOLCHANGE block emission, no style directive; FlatCAM RUST_PORT_LOGIC notes travel_coordinates and name-based toolchange special casing (marlin/repetier).
- **L3** (user decisions, verbatim): "1. Para E5.2 (`exc_areas` en el modelo + ruteo de travel), ¿dónde lo ponemos y cómo lo modelamos? ... — Document.exc_areas (Recommended)" "2. Para E5.4 (`toolchange_style` enum)... — Enum + directive + goldens (Recommended)"
