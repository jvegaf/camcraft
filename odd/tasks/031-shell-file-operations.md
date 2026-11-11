# shell-file-operations — new project and import

Feature document for the entry points the shell was missing: starting a fresh document with
its material defined, and importing a drawing (DXF/SVG/PDF/EPS/PNG) through the platform
picker. Read until `## Log` before touching code.

## Specs

- **S1 — new project.** `New` resets the drawing and opens the material dialog (`Novo trabalho —
  defina o material`), so the first thing defined is the stock.
- **S2 — import.** `Import` asks the host for a file and applies `ImportFile` with the path it
  returned; cancellation pushes nothing.
- **S3 — import options modal.** (follow-up) The dialog reports what it found and offers join/smooth/
  position/material-size options before importing.

## Tasks

| ID | Task | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered `4b2b188` — `NewProject` action + `Command::NewProject` + host-free reset + material dialog | S1 | parent | `4b2b188` |
| T2 | ✅ delivered `4b2b188` — `RequestImportFile` action + `Command::Import` wired to host picker | S2 | parent | `4b2b188` |
| T3 | ✅ delivered `a377741`, `248d28d` — import modal reports parse and offers tolerance/join/smooth/position/adjust-material options before applying | S3 | parent | `a377741`, `248d28d` |

## Log

- **L1 (owner, verbatim):** "faltan cosas como crear nuevo proyecto… importacion de dxf y svg…".
- **L2 (reconciliación, 2026-11-12):** T3 ya estaba implementado y cubierto por tests en `crates/cam-ui` (`a377741`, `248d28d`; suite `cargo test -p cam-ui --test shell` → 139 passed). Se actualiza el documento y el índice para reflejar S1–S3 entregadas.
