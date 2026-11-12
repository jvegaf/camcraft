# Mapa de olas: qué se puede construir en paralelo, y qué no

**Propósito.** Traducir el grafo de [docs/ROADMAP_CAMCRAFT.md](../../docs/ROADMAP_CAMCRAFT.md) y el
estado verificado de [index.md](index.md) §1 en un calendario de olas con agentes concretos,
**superficies de archivo reclamadas** y orden de merge. No es una lista de deseos: cada ola dice qué
archivos toca cada agente, porque el claim de la tarea 041 es el que impide que dos agentes se pisen,
y sólo puede impedirlo si la tarea declara sus superficies.

**Regla que ordena todo el documento.** El paralelismo no lo decide la cantidad de agentes, lo decide
el grafo de dependencias. Donde hay dependencia técnica no hay ola: hay serie.

---

## 1. Reglas de juego

1. **Un agente = un worktree = un claim.** Los worktrees los crea el humano desde `herdr` bajo
   `~/.herdr/worktrees/`; el tooling no crea ninguno y no asume `.worktrees/`. El claim se toma
   desde el worktree, con `just claim N`, y `scripts/claim-task.sh` compara **superficies**, no
   crates.
2. **La tarea declara sus superficies antes de empezar.** Un bloque `## Surfaces` en el documento de
   la tarea, con paths o globs relativos al repo. Sin ese bloque, el claim cae al modo viejo (tokens
   `crates/<name>` en el texto) y avisa.
3. **Nadie escribe `odd/tasks/INDEX.md` al reclamar.** El INDEX lo actualiza **quien mergea**, con
   `just deliver N <commit>`. Un claim que escribe el índice desde un worktree sucio no está en
   ninguna rama y deriva.
4. **`scripts/check.sh` verifica el claim.** Trabajar en un worktree vinculado sin claim falla el
   gate; trabajar en el árbol primario con cambios en `crates/` o `scripts/` también.
5. **Dueño único por archivo compartido.** `Cargo.toml` raíz y `justfile` tienen dueño en cada ola;
   los demás piden por mensaje, no editan.
6. **Máximo tres agentes por ola.** `target/` no es compartido: cada worktree compila el workspace
   entero (45 G en el árbol primario hoy). Cuatro agentes en paralelo es disco y CPU, no sólo
   coordinación.
7. **Merge en el orden que dice la ola**, y quien mergea actualiza el INDEX. Un merge fuera de orden
   reordena el índice de la siguiente ola, no el código.

---

## 2. Espina serial (lo que no se paraleliza)

```
E0.3 tolerancia ──► E3.2 multipasada ──► E4.2 fill ──► E4.3 índice ──► E4.4 rayon
E1.4 Gerber ─────► E3.1 unión de cobre ─┘
E1.1 modelo de placa (035) ──► todo test que construye un Document
```

- **E0.3** toca todas las operaciones geométricas (`offset.rs`, `region_offset.rs`, `lib.rs`). Va
  sola: en paralelo con cualquier otra cosa de `cam-geom` es un merge imposible.
- **E3.2 → E4.2 → E4.3 → E4.4** son cuatro pasos que se juzgan uno contra el otro. Ahí no hay ola.
- **F4 (NCC)** es la tesis del producto y tiene tres predecesores. Cualquier agente que empiece NCC
  antes de E3.1 está adivinando la interfaz de otro.
- **E1.1** (tarea 035, hoy `in progress`) es un cambio de raíz: cualquier test nuevo que construya un
  `Document` va a conflictear mientras esté abierto. Por eso la ola 1 espera su cierre.

---

## 3. Ola 1 — Instrumentos (3 agentes)

Objetivo: que a partir de acá toda afirmación sea medible. Es lo que `docs/ROADMAP_CAMCRAFT.md` §15
pide primero, y hoy está en 0 de 5.

| # | Ítem | Qué entrega | Superficies reclamadas | Depende de |
| --- | --- | --- | --- | --- |
| W1a | **E0.5 corpus + E0.4 arnés de paridad** | 6 placas de fabricación versionadas (`top/bottom/mask/silk/outline/drill`) y el arnés que compara conteo de polígonos, área total (ε relativo 1e-9) y puntos tras aplanar | `fixtures/corpus/`, `scripts/parity/`, `crates/cam-geom/tests/parity.rs` | — |
| W1b | **E0.8 CI** | `.github/workflows` con fmt + clippy + test + check-docs, y las recetas `just` correspondientes | `.github/`, `justfile`, `scripts/check.sh` | W1c y W1a para agregar sus jobs (mergea **último**) |
| W1c | **E0.7 benchmarks** | `criterion` en `cam-geom` y `cam-ops`, baseline guardado, job de regresión | `crates/cam-geom/benches/`, `crates/cam-ops/benches/`, `Cargo.toml` | — |

**Por qué W1a es un solo agente y una sola unidad:** un arnés sin corpus no juzga nada y un corpus
sin arnés es un ZIP de Gerbers. Juntos cierran además el criterio de salida de **E0.2**, que hoy es
inverificable por falta de corpus.

**Colisiones declaradas:** `justfile` es de W1b; si W1c necesita una receta, la pide, no la escribe.
`Cargo.toml` raíz es de W1c. `scripts/check.sh` es de W1b.

**Orden de merge:** W1c → W1a → W1b. El CI cierra la ola porque sus jobs necesitan lo que entregan
los otros dos.

---

## 4. Ola 2 — Importadores (4 agentes)

Objetivo: que una carpeta de fabricación real entre al modelo. Cuatro crates distintos, superficies
disjuntas: es la ola más limpia que tiene el roadmap.

| # | Ítem | Qué entrega | Superficies reclamadas | Depende de |
| --- | --- | --- | --- | --- |
| W2a | **E1.4 Gerber** | Trazo `D01` fuera de región, aperturas `O`/`RoundRect`, aperturas por macro, segundo `D02` dentro de `G36/G37` | `crates/cam-gerber/` | — |
| W2b | **E1.6 Excellon** | Ceros omitidos, dialecto completo, slots y tabla de herramientas probados contra archivos reales | `crates/cam-excellon/` | — |
| W2c | **E1.3 importador de carpeta** | Detectar y agrupar `top/bottom/mask/silk/outline/drill` por nombre y contenido; abrir la carpeta como placa | `crates/cam-import/` | W2a y W2b para el contenido, no para el agrupamiento |
| W2d | **E1.8 librería PCB** | Fresa de aislamiento como tipo real, V-bit con ángulo y punta end-to-end, `tools add` completo | `crates/cam-store/`, `crates/cam-cli/src/lib.rs` | — |

**Colisión declarada y cómo se resuelve:** W2d toca `cam-cli` (el `tools add`). Nadie más lo toca en
esta ola: el `import` de W2c ya existe y no cambia.

**Fuera de esta ola, y a propósito:** **E1.7 (reporte unificado `ImportReport`)** necesita a W2a y
W2b terminados, porque hoy cada crate expone sus propios errores
(`grep -rn ImportReport crates/cam-gerber crates/cam-excellon` → 0). Va como ola 2b de un solo
agente, después del merge de ambos.

---

## 5. Ola 3 — Geometría, en solitario (1 agente)

| # | Ítem | Qué entrega | Superficies reclamadas |
| --- | --- | --- | --- |
| W3 | **E0.3 política de tolerancia** | Un objeto de tolerancia explícito que cada operación recibe; ningún literal propio en operación pública | `crates/cam-geom/src/` |

**Nadie más toca `cam-geom/src/` en esa ventana.** Es el cambio de raíz del kernel: hacerlo en
paralelo con cualquier otra cosa geométrica garantiza conflicto semántico, no sólo textual. Si dos
agentes necesitan `cam-geom` a la vez, el segundo espera o pide la función ya escrita.

---

## 6. Ola 4 — Primer G-code PCB (2 agentes, con una preparación previa)

Objetivo: `cam-cli` generando un programa de placa. Hoy no hay `isolate`, ni drill desde tabla
Excellon, ni cutout de outline, ni el predicado de penetración (E2.4).

**Preparación obligatoria (1 agente, antes de abrir dos worktrees):** partir
`crates/cam-cli/src/lib.rs` en módulos por comando (`commands/import.rs`, `commands/drill.rs`, …). Hoy
es un archivo único de más de 500 líneas y **es el cuello de botella de todo paralelismo futuro**: dos
agentes que agregan dos comandos chocan en el mismo archivo aunque no compartan una línea de lógica.

| # | Ítem | Qué entrega | Superficies reclamadas | Depende de |
| --- | --- | --- | --- | --- |
| W4a | **E2.1 + E2.2 + E2.4** | Drill desde tabla Excellon agrupado por diámetro, orden determinista con 2-opt por iteraciones, predicado de penetración como función pura | `crates/cam-ops/src/peck.rs`, `crates/cam-ops/src/penetration.rs`, `crates/cam-cli/src/commands/drill.rs` | W3 (tolerancia), W2b |
| W4b | **E2.3 + E2.5** | Cutout desde el outline con tabs y mouse-bites, y el programa completo taladros + cutout por CLI con post GRBL | `crates/cam-ops/src/follow.rs`, `crates/cam-cli/src/commands/cutout.rs`, `crates/cam-post/` (datos) | W3, W1a (para juzgarlo) |
| W4c | **E3.1 + E3.2** | Unión de cobre previa y multipasada acumulada con la ley de offset y dirección por tipo de fresado | `crates/cam-ops/src/isolation.rs` | W3, W2a, W1a |

W4a y W4b son paralelos **sólo** si la preparación de `cam-cli` ya está mergeada; si no, la colisión
en `cam-cli/src/lib.rs` es segura. W4c comparte `cam-ops` con W4a: se reclama `crates/cam-ops/src/isolation.rs`
como superficie nueva y ninguno toca el archivo del otro.

---

## 7. Lo que queda detrás de la espina

Nada de esto tiene ola asignada, y no es un olvido: **F4 (NCC), E4.3, E4.4, E5.3 (autolevelling),
E6.x y E7.x** dependen de que existan E3.1/E3.2 y de que el arnés de W1a pueda medirlos. Asignarles
agente hoy sería asignarles una interfaz imaginaria.

Lo único de las fases tardías que se puede adelantar sin riesgo es **E5.5 (escritores Gerber/Excellon
en `cam-export`)**: no depende de nada del kernel, sólo de que W2a/W2b fijen el dialecto que se
escribe. Cabe como ola 2c de un agente, con superficie `crates/cam-export/` y el round-trip contra
`cam-gerber` y `cam-excellon` como aceptación.

---

## 8. Cómo se abre cada ola, en concreto

```bash
# 1. El humano crea el worktree desde herdr (no con `just worktree`).
#    Ruta: ~/.herdr/worktrees/<slug>, rama feat/<NNN>-<slug>.

# 2. Desde ese worktree, el agente reclama la tarea.
cd ~/.herdr/worktrees/<slug>
just claim 042          # falla si otra tarea ya reclamó una superficie superpuesta

# 3. Trabaja, y antes de pedir merge:
just check              # incluye la verificación del claim desde este worktree

# 4. Quien mergea, actualiza el índice y libera el claim.
just deliver 042 <commit>
```

`scripts/claim-task.sh` imprime las superficies que quedaron reclamadas. Si dos tareas se pisan, el
mensaje dice **qué superficie y de quién**: sin eso, el agente tiene que adivinar, y adivinar es lo
que la 041 vino a eliminar.
