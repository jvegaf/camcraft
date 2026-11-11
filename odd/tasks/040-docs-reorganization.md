# Reorganización de la documentación: README de camcraft y `odd/` ordenado

Documento de la feature. El repo se llama camcraft y es un CAM de PCB en Rust; la documentación
todavía cuenta la historia del producto anterior y `odd/` no se puede navegar a ojo.

## Specs

**S1** (owner, README): "tenemos que actualizar el readme del repo adaptandolo al nuevo proyecto."

**S2** (owner, el roadmap): "tenemos un roadmap en un solo fichero inmensamente grande, que un humano
no es capaz de gestionar."

**S3** (owner, `odd/tasks`): "hay ya una cantidad de ficheros enorme dentro de `odd/tasks`, junto con
ningun orden, por lo que deberiamos de decidir el nombrado del fichero de tal manera que esten
ordenados."

**S4** (owner, delega la decisión): "ya te dejo a ti la decision, si te viene bien numeral, por fecha,
carpetas... lo que mas me guste."

**S5** (owner, el criterio de aceptación): "pero necesito que sea mas ordenado para mi."

**S6** (decisión del orquestador, derivada de S3–S5):

- `odd/tasks/NNN-slug.md`, tres dígitos desde `001`, asignados en orden de **primer commit**
  (`git log --diff-filter=A --format=%ad --date=short`, desempate por el nombre anterior en orden
  alfabético). El número es **inmutable**: una tarea nueva toma el siguiente libre, ninguna se
  renumera. El slug no cambia.
- `odd/tasks/INDEX.md` es la tabla que se lee: `# | tarea | estado | fecha | último commit`.
- `odd/roadmap/index.md` desaparece partido en `odd/roadmap/`: `index.md` (lo que se lee, corto),
  `m00`..`m15` + `region-booleans.md` (la evidencia, verbatim), `decisions.md` (§7), `log.md` (§8).
- `odd/archive/pre-pivot.md` guarda §1–§3 de `odd/roadmap/index.md` (producto, stack y arquitectura del
  proyecto anterior), porque `docs/SYSTEM_DESIGN_CAMCRAFT.md` los reemplaza.
- Los enlaces se verifican con un chequeo ejecutable de targets relativos de markdown, no a ojo.
- `docs/old/` es una foto congelada: sus enlaces quedan como están y `odd/README.md` lo dice.

## Tasks

| # | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Renombrar las 39 tareas a `NNN-slug.md` y generar `INDEX.md` | S3, S4, S6 | inline (script) | `31887f9` |
| T2 | Actualizar las referencias a `odd/tasks/*` fuera de `odd/` | S3, S6 | inline (script) | `31887f9` |
| T3 | Partir `odd/roadmap/index.md` en `odd/roadmap/` + `odd/archive/` y actualizar sus referencias | S2, S6 | worker | |
| T4 | Escribir `odd/README.md`: el esquema de nombrado y cómo se lee `odd/` | S5, S6 | inline | `4e68e2b` |
| T5 | Reescribir `README.md` para camcraft (PCB), sin residuos del producto anterior | S1, S5 | inline | `4e68e2b` |

## Log

**L1** (owner, pedido original, verbatim): "te voy a meter una tarea facilita pero necesaria. por un
lado tenemos que actualizar el readme del repo adaptandolo al nuevo proyecto. por otro lado, no se si
recuerdas que ayer te pregunte por las odd tasks, y es que lo que me ocurre son dos cosas. tenemos un
roadmap en un solo fichero inmensamente grande, que un humano no es capaz de gestionar. por otro lado
hay ya una cantidad de ficheros enorme dentro de @odd/tasks, junto con ningun orden, por lo que
deberiamos de decidir el nombrado del fichero de tal manera que esten ordenados, ya te dejo a ti la
decision, si te viene bien numeral, por fecha, carpetas... lo que mas me guste. pero necesito que sea
mas ordenado para mi."

**L2** (los cortes, y el desvío de la regla de los 400): el primer corte es `31887f9` — cuarenta y
tantos archivos, pero **casi todo son movimientos**: el rename en sí no se puede partir sin dejar el
repo con dos convenciones a la vez, que es exactamente lo que este documento viene a eliminar. El
segundo corte es `4e68e2b`, sólo documentación nueva. El conteo de líneas del harness cuenta un `git mv`
como un borrado más una adición, así que 782 líneas "autoradas" no son 782 líneas de prosa: son ~130
de texto nuevo y el resto renombres. Los cortes quedan por unidad de trabajo, no por número.

**L3** (hallazgo que **no** es de esta tarea): el renombrado del repo a camcraft **no llegó al código**.
Quedan strings `campilot` en `crates/cam-desktop/src/main.rs:23` (el título de la ventana), `:27` (el id
de la app, que decide el directorio de datos) y `:335`, y en los directorios temporales de seis tests de
`cam-cli`. No se toca acá: cambiar el id de la app mueve el directorio de datos de alguien, así que
merece su propia tarea y su propio review.
