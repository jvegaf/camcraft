# offset-exact

Make the offset keep its arcs when a contour self-intersects. Feature G3.2 of
`docs/GAPS_CAMCRAFT.md` — the second hard blocker after G0.1, on the critical path to
isolation and NCC. Without it, every operation that offsets copper near a tight
clearance either loses exactness or refuses.

## Specs

**S1 — El offset actual no soporta isolation confiable.** De `docs/GAPS_CAMCRAFT.md:160-167`,
verbatim:

"- **G3.2 — El offset actual no soporta isolation confiable.**
  `offset_contour` (`cam-geom/src/offset.rs:69`) hace offset por curva y resuelve vértices
  intersectando curvas soporte, con fallback a puente recto + `OffsetWarning::MiterLimited`
  (`offset.rs:246-279`); el cleanup de self-intersection **tessela, parte y se queda con el loop más
  grande, reportando `Flattened`** (`offset.rs:403-491`); y `Region::offset` puede rechazar el
  resultado si el offset atraviesa la forma (`region_offset.rs:120-185,244-344`).
  **Impacto:** en cobre denso y concavo, aclaraciones finas de PCB, este camino o pierde exactitud
  (aplana arcos) o rechaza. Es el segundo bloqueante duro después de G0.1."

**S2 — Una regla no está fijada si sólo se probó en la geometría fácil.** El defecto vive en el camino
que se activa **sólo cuando el contorno se auto-intersecta**: en la geometría simple el arco sobrevive y
el resultado sale `flattened: false`. Un test que no fuerce la auto-intersección sobre un contorno con
arcos no prueba nada de esto, y es la misma clase de punto ciego que dejó pasar el defecto de
`dedup_pieces` en el motor de conjuntos.

**S3 — El motor de conjuntos ya sabe hacer esto sin aplanar.** `normalize` y `combine`
(`crates/cam-geom/src/boolean.rs`) preservan `Segment::Arc` y está fijado por tests. Si el cleanup puede
apoyarse en el arrangement en vez de en una polilínea muestreada, la pieza existe y G3.2 es reemplazar un
camino por otro, no inventar geometría.

## Tasks

| ID | Task | Specs | Route | Commit |
| -- | ---- | ----- | ----- | ------ |
| T1 | Mapear el pipeline del offset y decidir si el arrangement reemplaza al tessellated cleanup | S1, S3 | explorer | — |
| T2 | El cleanup de auto-interseccion preserva arcos | S1, S3 | — | — |
| T3 | Tests con arcos que **fuerzan** la auto-interseccion | S2 | — | — |
| T4 | G3.2 corregida en `docs/GAPS_CAMCRAFT.md` | S1 | — | — |

**Fuera de alcance, declarado:** `Region::offset` rechazando el resultado cuando el offset atraviesa la
forma (`region_offset.rs`) es el otro extremo del mismo bloqueante y merece su propia tarea cuando T2
cierre; `OffsetWarning::MiterLimited` no cambia acá.

## Log

**L1** (owner, autorizacion): la recomendacion fue "agarrar **G3.2** antes que nada cosmetico. El offset
que aplana arcos es el proximo defecto de la misma familia que el de hoy —silencioso, geometrico, y en el
camino critico de la primera operacion que de verdad corta." El owner respondio: "perfecto, dale".

**L2** (owner, correccion de proceso): "no veo las tasks en odd". Tenia razon: los dialectos de Excellon
recien entregados vivian solo como entradas de Log en `c3-model.md`, sin fila en su tabla de tareas, y esta
feature no tenia documento. Se agregaron las filas E1/E2 con su spec S6 y se creo este documento antes de
escribir una linea de codigo de G3.2.
