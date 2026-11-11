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
