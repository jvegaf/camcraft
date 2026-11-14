# 052 — E0.3 política de tolerancia (W3)

Política de tolerancia única para el kernel geométrico: un objeto explícito que cada operación recibe, sin literales de tolerancia propios en operaciones públicas (W3 / E0.3).

## Surfaces

- `crates/cam-geom/src/`
- `crates/cam-geom/tests/`
- `crates/cam-geom/benches/`
- `crates/cam-app/`
- `crates/cam-cli/`
- `crates/cam-desktop/`
- `crates/cam-excellon/`
- `crates/cam-export/`
- `crates/cam-gerber/`
- `crates/cam-import/`
- `crates/cam-model/`
- `crates/cam-move/`
- `crates/cam-ops/`
- `crates/cam-sim/`
- `crates/cam-toolpath/`
- `crates/cam-ui/`
- `crates/cam-units/`
- `crates/cam-view/`
- `docs/`

> Nota: waves.md declara W3 con superficie `crates/cam-geom/src/`; se incluyen consumidores, tests, benches y docs porque el cambio de API de tolerancia los afecta y el criterio de salida exige el workspace en verde.

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md y completa w3»
- **S2 (pedido adicional, verbatim).** «hay que seguir lo indicado en @docs/SYSTEM_DESIGN_CAMCRAFT.md»
- **S3 (waves.md W3, verbatim).** «| W3 | **E0.3 política de tolerancia** | Un objeto de tolerancia explícito que cada operación recibe; ningún literal propio en operación pública | `crates/cam-geom/src/` |»
- **S4 (ROADMAP_CAMCRAFT.md E0.3, verbatim).** «| E0.3 | Política de tolerancia única: un objeto explícito que cada operación recibe, reemplazando los literales dispersos (`cam-geom/src/lib.rs:52-58`, `offset.rs:26-43`, `region_offset.rs:62-74`) | **M** | Ninguna operación geométrica pública crea un literal de tolerancia propio; test que verifica la política |»
- **S5 (SYSTEM_DESIGN_CAMCRAFT.md §5.2, verbatim).** «### 5.2 Política de tolerancia
Un solo objeto, explícito, por operación:

```rust
pub struct TolerancePolicy {
    pub chord: f64,             // error de cuerda al teselar, mm
    pub epsilon: f64,           // comparación de puntos y áreas
    pub arc: ArcPolicy,         // Preserve | Flatten { chord }
    pub simplify: Simplify,     // Off | Collinear { epsilon } | DouglasPeucker { epsilon }
}
```

Nada dentro del kernel crea una política por su cuenta. La política viaja en la clave de cache — dos políticas distintas producen artefactos distintos, que es exactamente lo que queremos.»
- **S6 (SYSTEM_DESIGN_CAMCRAFT.md I3, verbatim).** «**I3 — La tolerancia es un parámetro, nunca un literal ni un ambiente.** No hay `thread_local`, no hay»

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Inventariar API/literales de tolerancia en `cam-geom` y definir `TolerancePolicy` (+ tipos asociados) alineado con §5.2, con constructores/default canónico explícito | S3,S4,S5,S6 | inline | done | 1d170da |
| T2 | Adaptar operaciones públicas y tipos de `cam-geom/src` para recibir `TolerancePolicy` (o referencia) donde corresponda y eliminar literales de tolerancia propios en operaciones públicas; reemplazar dispersos señalados | S3,S4,S5,S6 | inline | done | 1d170da |
| T3 | Actualizar tests/benches de `cam-geom` y consumidores del workspace a la nueva API; mantener semántica equivalente | S1,S3,S4 | inline | done | 720285d, pendiente (consumidores) |
| T4 | Agregar test que verifique la política (ninguna operación geométrica pública crea literal propio / uso de política explícita) y ajustar docs si aplica | S4,S5 | inline | done | 720285d |
| T5 | Verificación (fmt, clippy, tests relevantes/workspace), reviews RDD por slice y cierre (log, commits) | S1,S4 | inline | in_progress | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md y completa w3»
- **L2 (pedido adicional, verbatim).** «hay que seguir lo indicado en @docs/SYSTEM_DESIGN_CAMCRAFT.md»
- **L3 (contexto).** Worktree `waves-w3` (rama `waves/w3`). Waves.md define W3/E0.3 con superficie `crates/cam-geom/src/`; ROADMAP cita literales dispersos en `cam-geom/src/lib.rs:52-58`, `offset.rs:26-43`, `region_offset.rs:62-74`. SYSTEM_DESIGN §5.2 define `TolerancePolicy` (chord, epsilon, arc, simplify) y exige que el kernel no cree política por su cuenta. Exploración inicial (manual tras timeout de subagente) muestra API pública con `tolerance: f64` en arc/chain/contour/segment/intersect/join/offset/region_offset/region/validate/winding/trim/distance/boolean, `OffsetOptions.tolerance`, constantes `DEFAULT_TOLERANCE`/`MAX_TESSELLATION_STEPS` y literales internos (p.ej. `OFFSET_CHORD_TOLERANCE`, `DISTANCE_SLACK`, tolerancias de teselado/comparación); uso extendido en consumidores y en tests/benches de cam-geom.
- **L4 (T1–T2, slice 1, `1d170da`).** Añadido `crates/cam-geom/src/tolerance.rs` con `TolerancePolicy { chord, epsilon, arc, simplify }`, `ArcPolicy`, `Simplify`, `DEFAULT_CHORD`/`DEFAULT_EPSILON`, constructores/helpers (`standard`, `uniform`, `of`, `with_*`, `with_default_*`) y `Default`. Reexportado en `lib.rs`. Migradas APIs públicas de `cam-geom/src` para recibir `TolerancePolicy` explícita (point/bounds/arc/segment/contour/chain/join/intersect/distance/validate/winding/trim/offset/region/region_offset/boolean/transform/primitive), `OffsetOptions::{new,standard}` (sin `Default` literal propio), reemplazo de literales dispersos señalados y ajuste semántico epsilon/chord; `primitive::ellipse` usa `chord`; `Contour::bounds()` exacto por segmentos.
- **L5 (RDD slice 1).** RDD lineage `review-1e4c77edb97c4de7` (lenses risk,reliability, base `398c29da03dfd6666412d54bec9ddc8fa5550033`, committed-only sobre slice 1): refuter halló R3-001 (CRITICAL, `ArcPolicy::Flatten` no se aplicaba en `offset_contour`). Corrección acotada en `crates/cam-geom/src/offset.rs` para respetar `options.tolerance.arc` (flatten forzado con chord de política y advertencia `Flattened`), validación dirigida aprobada; review aprobada y acknowledge completado. Hallazgos informativos no bloqueantes: R1-NAN-DEDUP, R3-002.
- **L6 (T3–T4, slice 2, `720285d`).** Migrados tests/benches de `cam-geom` a la nueva API de tolerancia explícita y añadido `crates/cam-geom/tests/tolerance_policy.rs` (defaults, epsilon/chord, aceptación de política en operaciones públicas y escaneo de literales propios en `crates/cam-geom/src` excluyendo `tolerance.rs`). `cargo test -p cam-geom --tests` verde (incluye `tolerance_policy` 7 passed).
- **L7 (RDD slice 2).** RDD lineage `review-f86ffe64c54ac0a1` (lenses readability,reliability, base `1d170da13611013a0d92d11e39928651c0fd458b`, committed-only sobre slice 2): review aprobada y acknowledge completado; hallazgos informativos no bloqueantes R2-001, R3-001.
- **L8 (T3/T5 en curso).** Pendiente commit de migración de consumidores del workspace y cierre documental/verificación final; lanzamiento de review RDD pendiente para el/los slice(s) restantes.
