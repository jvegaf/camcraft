# c1-booleans

Motor de conjuntos sobre el arrangement que ya existe. Es C1 del
`docs/SYSTEM_DESIGN_CAMCRAFT.md` §14, la fundación que bloquea la geometría Gerber (la polaridad
dark/clear es una resta), isolation, NCC y el predicado de penetración del oráculo.

## Specs

**S1 — Unión, intersección y diferencia entre dos `Region`, con arcos preservados.**
> "**I2 — Nada se aplana en silencio.** Los arcos son *first-class* de punta a punta. Toda operación
> declara `ArcPolicy::Preserve` o `ArcPolicy::Flatten { chord }`, y el aplanado explícito es visible en
> el artefacto que lo produce. Un aplanado implícito es un bug, no una optimización."

**S2 — Criterio de salida medible.**
> "200 operaciones booleanas contra áreas conocidas con error < tolerancia; `proptest` sobre regiones
> aleatorias incluidas degeneradas; corpus sin errores de normalización"

**S3 — La robustez de entrada es parte de C1, no una fase posterior.**
> "`E0.2` — Robustez de entrada: normalización de contornos auto-intersectados, bordes coincidentes,
> vértices de contacto"

> **Corrección medida (C1b):** el GAPS afirmaba que `region.rs:162-165` rechaza los contornos
> auto-intersectados. **No los rechaza**: `Region::new` sólo exige que el contorno sea cerrado, y
> `normalize` los pasaba tal cual, dejando un contorno que se cruza con área con signo cero y material
> distinto de cero. C1b corta cada contorno contra sí mismo y la figura de ocho se resuelve en sus dos
> loops."

**S4 — El álgebra del conjunto se testea como álgebra, no sólo caso por caso.**
Las identidades que cualquier motor de conjuntos debe cumplir son el oráculo barato: conmutatividad de
`∪` y `∩`, idempotencia, `A ∖ A = ∅`, y sobre todo **`|A ∪ B| + |A ∩ B| = |A| + |B|`**. Una operación que
pasa casos sueltos y rompe esa identidad está mal aunque cada test individual pase.

**S5 — Determinismo.** Dos corridas del mismo par de regiones producen el mismo resultado byte a byte, y
el resultado no depende del orden en que se enumeran los contornos de entrada (I6 del diseño).

## El mapa medido

`crates/cam-geom/src/boolean.rs` (596 líneas) expone **una sola función pública**:

```rust
pub fn normalize(region: &Region, tolerance: f64) -> Result<Normalized, GeomError>
```

Todo el aparato está construido y es privado:

| Pieza | Línea | Qué hace |
| --- | ---: | --- |
| `cut_fractions` | 152 | dónde se cruzan los contornos, en fracciones por segmento |
| `dedup` / `split_at` / `sub_segment` | 222-250 | corta sin perder arcos |
| `material_side` | 306 | clasifica un tramo según qué lado tiene material |
| `material_beside` | 372 | la paridad en un punto de prueba |
| `assemble` | 391 | encadena los supervivientes girando en sentido horario en cada cruce |
| `angular_order` / `outward_angle` | 516-528 | el criterio de giro de la caminata |
| `exactly` / `survives_the_bulge` | 561-588 | qué arco no cabe en el modelo y hay que aplanar, reportándolo |

`Region` expone `new`, `contains`, `distance_to_boundary`, `subtract(&Contour)` y `normalize`.

**Lo que falta, exactamente:**

1. **Un arrangement de dos regiones.** `normalize` corta los contornos de una región *contra sí mismos*.
   Una operación de conjunto necesita cortar los de A contra los de B, en un solo arrangement.
2. **El predicado sobre `(en A, en B)`.** `material_beside` responde por una región; falta preguntarle lo
   mismo a las dos y aplicar `|`, `&`, `& !`.
3. **La orientación del resultado.** Para la diferencia, los tramos de B entran invertidos — es el caso
   "hueco dentro de A". La caminata orienta con material a la izquierda, así que B aporta al revés.
4. **Bordes coincidentes.** Cuando un contorno de A y uno de B yacen exactamente uno sobre otro, un punto
   de prueba sobre el borde no dice de qué lado hay material. Es el caso que rompe clasificadores
   ingenuos y el que el corpus de placas va a traer.
5. ~~Aceptar la entrada degenerada.~~ **Corregido en C1b:** `Region::new` **no** rechaza contornos
   auto-intersectados — sólo exige que estén cerrados — y `normalize` los dejaba pasar sin resolver.
   Ahora cada contorno se corta también contra sí mismo, y el auto-cruce se convierte en dos loops.

## Tasks

| ID | Tarea | Spec | Criterio de salida |
| --- | --- | --- | --- |
| C1a | `union`, `intersection`, `difference` entre dos `Region` sobre el arrangement existente | S1, S2, S4, S5 | 200 operaciones contra áreas conocidas con error < tolerancia; la identidad `\|A∪B\| + \|A∩B\| = \|A\| + \|B\|` en todas; arcos preservados salvo aplanado reportado |
| C1b | Robustez de entrada: auto-intersección aceptada y normalizada, bordes coincidentes, vértices de contacto | S1, S3 | El corpus de entradas degeneradas produce resultados en vez de errores, y cada una sigue cumpliendo la identidad de área |
| C1c | `proptest` sobre pares de regiones aleatorias, degeneradas incluidas | S2, S4 | Conmutatividad, idempotencia, De Morgan y la identidad de área, con presupuesto de casos fijo en CI |

## Log

**L1** (usuario, autoriza): "vale" — sobre la propuesta de traer el mapa de `boolean.rs` antes de escribir
y cortar C1 en tres pedazos con criterios de salida medibles.

**L2** (medido): `boolean.rs` tiene 596 líneas y **una** función pública, `normalize`. El corte por
fracciones, la clasificación por lado material, el ensamblado con giro horario y el aplanado reportado ya
existen; lo que no existe es el arrangement de dos regiones, el predicado sobre `(en A, en B)`, la
inversión de B para la diferencia, el manejo de bordes coincidentes y la aceptación de entrada
auto-intersectada. Es decir: **el motor está a medio camino entre "sabe limpiar una región" y "sabe
combinar dos"**, y el trabajo es la bisagra.

**L3** (evidencia — la correccion mas util de C1b, y no era de codigo): la especificacion citaba al GAPS
diciendo que `region.rs:162-165` **rechaza** los contornos auto-intersectados. Leido el codigo, no los
rechaza: `Region::new` solo exige que el contorno este cerrado, y `normalize` documentaba explicitamente
que un contorno que se cruza "se pasa tal como llego". O sea que una figura de ocho era un region legal,
con paridad que respondia bien por cada punto, **y un area con signo de cero mientras su material no lo
era**: el motor mentia en silencio sobre cualquier entrada auto-intersectada.

Arreglado cortando cada contorno tambien contra si mismo, en `cut_fractions` y en `combine`. La figura de
ocho se resuelve en dos loops de area 1 cada uno, sus identidades basicas se cumplen, y los 83 tests
previos de `cam-geom` siguen verdes: el auto-corte no molesta a la entrada sana.

Cubierto tambien en el mismo corte: un vertice de contacto da interseccion vacia (un punto no es area) y
una arista compartida no se cuenta dos veces (el dedup por pieza identica, que ya existia desde C1a).

**L4** (un defecto que C1 no cubria, encontrado por una placa y no por sus tests): el 11 de noviembre,
construyendo el flash de una apertura circular de Gerber, `combine` rechazo un circulo con "the arrangement
revisits a piece, so its pieces cannot be chained closed". El circulo no tenia nada de malo:
`primitive::circle` emite dos semicircunferencias correctas, con area exacta pi, y `normalize` lo dejaba
intacto. El defecto estaba en `dedup_pieces`, la funcion que C1a introdujo para matar el bug de la arista
doblada: agrupaba por **par de extremos**, y las dos semicircunferencias de un mismo circulo comparten
exactamente los dos puntos de costura. Borraba una, y el conjunto resultante ya no cerraba — el motor
tenia razon al negarse.

**Por que los tests no lo vieron, que es la parte que importa.** Las cinco propiedades de `C1c` corrian
sobre 640 pares de regiones, pero los generadores eran estrellas de radio aleatorio y tiras de 20 mm por
0,02 mm. Ninguno de esos pares es un **contorno cerrado hecho solo de arcos**, y esa es justamente la forma
que una placa usa todo el tiempo: un pad, un agujero, un anillo de aislamiento. Los discos que ya
existian se intersecaban siempre, y la interseccion corta las piezas en los puntos de cruce, que destruye
la identidad de la costura. El caso que falla es el que **no corta nada**: un disco contenido, o una union
contra la region vacia.

**Arreglo (`c3c1fb0`).** Dos piezas son la misma arista solo si son la misma curva recorrida en el mismo
sentido. La clave canonicaliza los extremos pero **acarrea la direccion con el intercambio** —si invertís
el orden, negás el signo del sweep— y agrega la geometria cuantizada del arco (centro, radio, sweep con
signo). `A ∪ A` sigue colapsando como debe; un disco queda entero. Los dos tests nuevos mueren cuando se
anula el sweep en la clave, que es la prueba de que muerden.

**Correccion al informe de C1.** Al cerrar C1 escribi que el motor de conjuntos estaba completo. No lo
estaba: estaba completo respecto de lo que sus tests miraban. La propiedad "el area de la union mas el area
de la interseccion da la suma" era cierta para estrellas y tiras, y falsa como afirmacion sobre el motor,
porque un contorno de arcos ni siquiera podia entrar. G0.1 se reabre con nombre propio.

**Limitacion que el arreglo no toca:** un disco que **toca** desde adentro el borde de la region, sin
cruzarlo, seguia rechazandose ("revisits a piece") a tolerancia 1e-9 en el momento de L4 (ver L5).

**L5** (corte 2026-11-12, baseline + C1b tangencia + refuerzo C1c/C1a): baseline `cargo test -p cam-geom` en verde (booleanos 15+5 y suite completa). C1b: `push_fractions` en `crates/cam-geom/src/boolean.rs` ahora parte también en contactos `Tangent` interiores, de modo que un toque sin cruce genera vértice en el arrangement; se fijó con `a_disc_tangent_to_the_inside_of_an_edge_resolves` en `crates/cam-geom/tests/boolean_operations.rs` (unión/intersección/diferencia e identidad de área). C1c/C1a: se agregaron propiedades explícitas de De Morgan (`de_morgan_difference_over_union`, `de_morgan_difference_over_intersection`) en `boolean_properties.rs` y un corpus determinista de 210 operaciones contra áreas analíticas conocidas en `two_hundred_known_area_operations_hold`. Quedan por acreditar/cerrar según el texto literal del doc: corpus amplio de normalización/degeneradas y cualquier otro caso no resoluble que aparezca; los cambios de código/tests quedan sin commitear en esta sesión.
