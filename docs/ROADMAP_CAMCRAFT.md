# Roadmap y plan de ejecución: camcraft

**Propósito.** Convertir el diagnóstico de [GAPS_CAMCRAFT.md](GAPS_CAMCRAFT.md) y las decisiones
D1–D6 asentadas allí (§6) en un plan ejecutable: fases con dependencias explícitas, tareas acotadas y
**criterios de salida medibles**. Este documento reemplaza el orden de fases de
[Plan de camcraft.md](Plan%20de%20camcraft.md) donde ambos difieren; el plan original sigue siendo la
referencia de alcance funcional.

**Base de decisiones.** Motor de conjuntos sobre el kernel propio (D1). Aceptación híbrida por capa
(D2). Schema `c2d/document@3` aditivo con la semántica PCB en metadata (D3). Dominio de madera
congelado y fuera de la UI principal (D4). Oráculo de dos niveles (D5). Formato `.FlatPrj` fuera de
alcance (D6).

---

## 1. Principios de ejecución

Estos principios no son ceremonia: cada uno responde a un modo de falla concreto de un port de CAM.

1. **El oráculo antes que la operación.** Ninguna operación PCB se considera terminada sin el
   instrumento que la juzga ya existente. Escribir isolation antes del predicado de penetración es
   escribir código que no se puede aprobar ni rechazar.
2. **CLI primero, UI al final.** Todo el pipeline se ejercita por `cam-cli` (15 subcomandos, exit
   codes estables) y `script`. La UI es una capa encima de operaciones ya probadas, nunca el lugar
   donde se descubre si funcionan.
3. **La fase 0 no se saltea.** El motor de conjuntos, la política de tolerancia, el corpus y los
   benchmarks son cimiento, no preparación.
4. **Dos kernels de tolerancia, una política.** Toda operación geométrica declara qué tolerancia de
   cuerda usa y por qué. Sin eso no hay determinismo ni paridad reproducible.
5. **Slices de ~400 líneas con su test y su doc en el mismo commit**, en rama de feature. Un slice
   cuyo riesgo sea alto va en commit propio.
6. **Nada se descarta en silencio.** El patrón `ImportReport`/`Skipped { construct, reason,
   location }` (`cam-import/src/report.rs:24-115`) se hereda para Gerber/Excellon y para cualquier
   geometría que el kernel rechace.

---

## 2. Mapa de fases y dependencias

```
F0 Fundamentos ──┬──► F1 Modelo @3 + importadores ──┬──► F2 Drilling + Cutout ──┐
   (bloqueante)  │                                   │                          │
                 └───────────────────────────────────┘                          ├──► F5 G-code PCB
                                                     └──► F3 Isolation ──► F4 NCC ┘
                                                                              │
                                                          F6 UI PCB ◄─────────┘
                                                                              │
                                                          F7 Extensión ◄──────┘
                                                                              │
                                                          F8 Publicación ◄────┘
```

`E0.1` (motor de conjuntos) bloquea `F3` y el cierre de `F1`. `E0.3` (arnés de paridad) y `E0.4`
(benchmarks) pueden avanzar en paralelo con `E0.1` desde el día uno, y son los que permiten que `F1`
y `F4` tengan criterio de aceptación.

---

## 3. Fase 0 — Fundamentos

**Objetivo.** Que exista el instrumento antes que el producto.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E0.1 | Motor de conjuntos sobre el arrangement existente: `union`, `intersection`, `difference` entre `Region`s, con predicados de selección de tramos sobre `material_side`/`material_beside` (`boolean.rs:306,372`) | **L** | 200 operaciones booleanas contra áreas conocidas con error < tolerancia; property tests con `proptest` sobre regiones aleatorias (incluidas degeneradas) |
| E0.2 | Robustez de entrada: normalización de contornos auto-intersectados, bordes coincidentes, vértices de contacto (`region.rs:162-165` hoy rechaza) | **M** | El corpus de E0.3 no produce errores de normalización |
| E0.3 | Política de tolerancia única: un objeto explícito que cada operación recibe, reemplazando los literales dispersos (`cam-geom/src/lib.rs:52-58`, `offset.rs:26-43`, `region_offset.rs:62-74`) | **M** | Ninguna operación geométrica pública crea un literal de tolerancia propio; test que verifica la política |
| E0.4 | Arnés de paridad **geométrica**: conteo de polígonos, área total (ε relativo 1e-9), conteo de puntos tras aplanar, sobre corpus fijo | **M** | Corre en CI sobre el corpus mínimo y falla ante divergencia |
| E0.5 | Corpus mínimo de placas: KiCad 2 capas simple, KiCad 4 capas con pours, EasyEDA, Eagle, Altium, y una placa grande >100×100 mm con pours — cada una con `top/bottom/mask/silk/outline/drill` | **M** | 6 placas versionadas con sus artefactos de fabricación |
| E0.6 | Baseline de performance de FlatCAM ejecutando el corpus (entorno Python separado; los números se guardan como goldens versionados) | **M** | Tabla de tiempos y conteo de movimientos por placa y por operación, reproducible desde un script |
| E0.7 | Benchmarks propios con `criterion` + job de CI que falla ante regresión | **S** | `just bench` corre y CI compara contra el baseline |
| E0.8 | CI real (`.github/workflows`): fmt + clippy + test + paridad + bench | **S** | Un PR sin CI verde no se mergea |

**Riesgo propio de F0.** `E0.2` es donde se concentra el riesgo del proyecto. La mitigación no es
más diseño, es corpus y property tests tempranos. Si `E0.1`+`E0.2` no cierran con el corpus, se
activa el umbral de abandono de D1 y se reevalúa `i_overlay` con aplanado explícito — decisión que se
toma con evidencia, no con intuición.

---

## 4. Fase 1 — Modelo de placa `@3` e importadores

**Objetivo.** Abrir un proyecto de fabricación y ver el cobre.

**La regla de D3 manda:** la semántica PCB vive en metadata. El kernel no gana variantes geométricas
nuevas; el modelo gana roles, origen y operaciones.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E1.1 | `c2d/document@3`: `LayerRole` (copper_top, copper_bottom, mask, silk, outline, drill), metadata de origen por entidad, migración `@2→@3` con defaults | **M** | Un documento `@2` carga, migra y se guarda como `@3` sin pérdida; test de round-trip |
| E1.2 | `Operation::{Isolation, NonCopperClear, Cutout, Paint, Panelize}` en el modelo, sin lógica todavía | **S** | El schema las persiste y `inspect` las reporta |
| E1.3 | Importador de **carpeta de fabricación** (detecta y agrupa top/bottom/mask/silk/outline/drill por nombre y contenido) | **S** | Una carpeta de KiCad se abre como placa completa con roles correctos |
| E1.4 | Parser Gerber RS-274X: aperturas estándar (C, R, O, P), macros, `G36/G37`, interpolación circular `G74/G75`, unidades y formato de coordenadas, `M02` | **L** | Las 6 placas del corpus producen geometría que pasa E0.4 contra FlatCAM |
| E1.5 | Resolución de polaridad dark/clear, step & repeat y regiones anidadas — **requiere E0.1** | **M** | Área y conteo de polígonos dentro de tolerancia en las capas con clears |
| E1.6 | Parser Excellon: tabla de herramientas, coordenadas, unidades, ceros omitidos, slots | **M** | Los 6 archivos de taladro del corpus coinciden en herramienta, diámetro y cantidad de hits |
| E1.7 | Informe de importación Gerber/Excellon sobre el patrón existente | **S** | Una placa con un arco inválido importa el resto y reporta `Skipped` con ubicación |
| E1.8 | Librería de herramientas: V-bit con ángulo y punta, fresa de aislamiento, broca (verificar/cubrir G2.5) | **S** | Una V-bit del catálogo produce la profundidad esperada en isolation |

**Puerta de F1.** Las 6 placas abren, guardan y recargan. Ninguna capa pierde geometría sin reporte.
La paridad geométrica de E0.4 pasa en todas las capas.

**Nota de tolerancia de arcos.** FlatCAM aplana todo al pasar por shapely; nosotros preservamos arcos.
La comparación es de área y cobertura, nunca de conteo de vértices con FlatCAM. El aplanado propio es
explícito y controlado por operación (E0.3).

---

## 5. Fase 2 — Drilling y Cutout

**Objetivo.** El primer G-code real, punta a punta, con la maquinaria que ya existe.

Se eligen primero porque reutilizan lo que hay: `drill.rs` tiene peck/retract y drill bank
(`cam-toolpath/src/drill.rs:85-132`), `profile` existe (`lib.rs:483`) y `cam-post` ya emite G-code
con golden tests.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E2.1 | Drilling desde tabla Excellon: agrupado por diámetro, selección por herramienta, slots como taladros o como trayectorias | **M** | Las 6 placas emiten un programa con la cantidad de hits y cambios de herramienta correctos |
| E2.2 | Ordenamiento determinista por herramienta (vecino más cercano + 2-opt con presupuesto de **iteraciones**, no de tiempo — invariante de determinismo) | **M** | Dos corridas idénticas producen G-code byte a byte igual |
| E2.3 | Cutout con tabs: perfil exterior, gaps y mouse-bites sobre el `outline` de la placa (verificar tabs en `profile`, G3.6) | **M** | Una placa con tabs se corta completa sin soltar la pieza; simulación sin colisión |
| E2.4 | Predicado de penetración (`_cut_model_overlaps_copper`) como función pura con tests | **S** | Detecta penetración inyectada artificialmente; corre en CI |
| E2.5 | Programa completo taladros + cutout por CLI con preprocesador GRBL | **S** | `cam-cli` genera y `cam-sim` acepta |

**Puerta de F2 = Hito 2 del plan original.** Primer G-code de drilling y cutout validado en
simulación sobre las 6 placas.

---

## 6. Fase 3 — Isolation

**Objetivo.** El primer producto que un usuario de PCB quiere, y el que más exige del kernel.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E3.1 | Unión previa del cobre de la capa (requiere E0.1) | **M** | La unión de una capa del corpus coincide en área con FlatCAM dentro de tolerancia |
| E3.2 | Offset multipasada acumulado: pasada `i` a distancia `dia·((2i+1)/2) − i·overlap·dia`, dirección por tipo de fresado | **L** | 3 pasadas con overlap 0.5 producen anillos sin colisión entre sí y sin gouge |
| E3.3 | Profundidad derivada del V-bit (ángulo y punta) | **M** | Una V-bit de 30° y 0.2 mm de punta da profundidad calculada; test con el valor esperado |
| E3.4 | Guarda anti-gouge: si el ratio de penetración supera el umbral, reparar o fallar con diagnóstico | **M** | Inyectar un caso de penetración hace fallar la operación con mensaje accionable |
| E3.5 | `iso_type` exterior/interior/completo y opción de conservar pours | **M** | Selector verificado en una capa con pours del corpus |
| E3.6 | Comando `cam-cli isolate` completo + preprocesador | **S** | G-code generado desde CLI, aceptado por `cam-sim` |

**Riesgo de F3.** El offset actual aplana al limpiar self-intersections (`offset.rs:403-491`) y puede
rechazar el resultado (`region_offset.rs:120-185`). Si `E3.2` no cierra con calidad suficiente sobre
cobre denso, la salida es mejorar el cleanup del offset (arrangement-based) **antes** de avanzar a F4
— no rodearlo con heurísticas en la operación.

---

## 7. Fase 4 — Non-Copper Clear (rendimiento)

**Objetivo.** La operación que define la promesa del producto. **Nada de esto se escribe antes de
que `E0.6`+`E0.7` estén en verde**, porque sin baseline la promesa no es medible.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E4.1 | Envolvente de no-cobre a partir del cobre unido, con margen y offset (dirección "a la izquierda" del material, semántica de FlatCAM) | **M** | Cobertura del área de no-cobre dentro de tolerancia en el corpus |
| E4.2 | Algoritmo estándar de relleno por offsets concéntricos sobre región con islas | **L** | Sin gouge ni áreas sin limpiar en el corpus |
| E4.3 | Índice espacial (`rstar` o grilla propia) para intersección y vecindad durante el relleno | **M** | Tiempo de E4.2 baja de forma medible contra la versión ingenua (se guarda el número) |
| E4.4 | Paralelismo por isla con `rayon` | **L** | Speedup medido en la placa grande; salida **determinista** byte a byte |
| E4.5 | Rest machining entre herramientas (grueso→fino), reusando la región restante | **L** | Dos herramientas cubren el área sin recortes redundantes; medido |
| E4.6 | Métodos alternativos: seed y lines | **M** | Los tres métodos producen salida válida |
| E4.7 | Benchmark publicado de NCC vs FlatCAM en la placa grande | **S** | **Hito 3: NCC más rápida que FlatCAM en la placa grande**, reproducible desde CI |

**Riesgo de F4.** Es la fase donde el paralelismo puede romper el determinismo (orden no
determinista de reducción). La mitigación es reducir con orden estable siempre, y test de igualdad
byte a byte con distintos `RAYON_NUM_THREADS`.

---

## 8. Fase 5 — G-code de PCB

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E5.1 | Catálogo de dialectos `.pp` de FlatCAM como datos (GRBL, Marlin, Repetier y los de uso real) | **M** | Cada dialecto con golden test byte a byte |
| E5.2 | `exc_areas` en el modelo + ruteo de travel alrededor de exclusiones | **M** | Un travel que cruza un área prohibida se desvía; simulación sin colisión |
| E5.3 | Autolevelling como transformación del `MoveStream`: mapa de alturas, segmentación de arcos y movimientos largos | **L** | Sobre una placa con warp simulado, la profundidad efectiva queda dentro de tolerancia |
| E5.4 | `toolchange_style` como enum en el preprocesador, reemplazando el matching por nombre de FlatCAM (`CNCJobObject.py:947-962`) | **S** | Los dialectos con M6 especial emiten igual que el original; golden |
| E5.5 | Escritores Gerber y Excellon en `cam-export` | **M** | Round-trip: exportar e importar recupera la geometría dentro de tolerancia |

---

## 9. Fase 6 — UI PCB

**Objetivo.** Un flujo completo sin tocar `cam-cli`. La UI es el crate más grande (19k líneas) y ya
tiene shell, capas, ribbon, editor de estrategia, librería de herramientas, preview 3D y picking: el
trabajo es de formularios y de identidad de placa, no de infraestructura.

| ID | Tarea | Tamaño | Criterio de salida |
| --- | --- | --- | --- |
| E6.1 | Capas con identidad de placa: color por rol, visibilidad, modo cobre | **M** | Las 6 capas se distinguen visualmente y se alternan |
| E6.2 | Árbol de proyecto: placa → capas → operaciones | **M** | El árbol refleja el documento y navega a la operación |
| E6.3 | Formularios de isolation, NCC, drilling, cutout y paint sobre el editor de estrategia existente | **L** | Cada operación se configura sin CLI |
| E6.4 | Panel de herramientas ligado a la librería SQLite (no a un snapshot) | **M** | Editar una herramienta se refleja en las operaciones |
| E6.5 | Atajos de teclado y Ctrl+S (verificar G5.3) | **S** | Guardar sin diálogo; mapa de atajos documentado en la ayuda |
| E6.6 | Previsualización de toolpaths y simulación integradas | **M** | El playback muestra gouge/uncut del programa |

**Puerta de F6 = Hito 4 del plan original.** Flujo completo en la interfaz: importar, configurar,
generar, simular, exportar.

---

## 10. Fase 7 — Extensión

Sólo después de que isolation y NCC estén estables y medidos.

| ID | Tarea | Criterio de salida |
| --- | --- | --- |
| E7.1 | Paint (relleno de áreas arbitrarias, tres métodos, rest por herramienta) | Área pintada cubierta sin gouge |
| E7.2 | Panelize (matriz rows×cols con spacing, esquinas y constraints) | Placa panelizada con G-code único |
| E7.3 | Film y punch | Artefactos de fabricación generados |
| E7.4 | Copper thieving (opcional, según demanda) | Área de cobre por cm² configurable |

`nest`/`nest_profile` de `cam-geom` **no** son panelize: resuelven aprovechamiento de material, otro
problema. No se reciclan para E7.2.

---

## 11. Fase 8 — Endurecimiento y publicación

| ID | Tarea | Criterio de salida |
| --- | --- | --- |
| E8.1 | Corpus ampliado (20+ placas reales) y tests de regresión de geometría | El corpus corre en CI bajo un umbral de tiempo |
| E8.2 | Benchmarks publicados y reproducibles | Un tercero reproduce los números desde el repo |
| E8.3 | Empaquetado Windows + Linux | Instaladores firmados y probados |
| E8.4 | Documentación: formato, CLI, guía de migración desde FlatCAM | Un usuario nuevo genera G-code siguiendo la guía |
| E8.5 | Repositorio público: README, contribución, issues etiquetados, alfa para probadores | Alfa publicada |

---

## 12. Hitos de verificación

Los hitos son las puertas; si una puerta no abre, la fase siguiente no empieza.

| Hito | Criterio medible | Fase |
| --- | --- | --- |
| **M0** | Motor de conjuntos pasa 200 operaciones contra áreas conocidas + corpus de 6 placas versionado + CI verde | F0 |
| **M1** | Las 6 placas abren, se guardan y recargan; paridad geométrica en todas las capas | F1 |
| **M2** | G-code de drilling y cutout válido y aceptado por `cam-sim` en las 6 placas | F2 |
| **M3** | Isolation sin gouge en las 6 placas, con predicado de penetración en verde | F3 |
| **M4** | **NCC más rápida que FlatCAM en la placa grande**, determinista, reproducible desde CI | F4 |
| **M5** | G-code listo para máquina con compensación Z opcional y ruteo de exclusiones | F5 |
| **M6** | Flujo completo en la UI sin tocar `cam-cli` | F6 |

---

## 13. Riesgos y puntos de reevaluación

| Riesgo | Probabilidad | Impacto | Mitigación / punto de decisión |
| --- | --- | --- | --- |
| Robustez del arrangement propio ante degeneraciones | Alta | Bloquea F3 | Corpus + property tests en F0; umbral de abandono de D1 si no cierra |
| Offset aplana o rechaza en cobre denso | Media-Alta | Calidad de isolation | Mejorar cleanup arrangement-based en F3, no heurísticas en la operación |
| Paralelismo rompe determinismo | Media | Promesa del producto | Reducción con orden estable; test con distintos `RAYON_NUM_THREADS` |
| Booleanas con Gerber reales (la fuente de errores más frecuente de FlatCAM) | Alta | F1 y F3 | Corpus grande desde F0 es la única defensa real |
| Performance no alcanza a FlatCAM | Media | Tesis del producto | Baseline medido en F0; si M4 no abre, el problema es de algoritmo y hay que verlo antes de F6 |
| Alcance: el dominio de madera consume foco | Media | Cronograma | D4: congelado y fuera de la UI principal |
| Undo por snapshot con documentos grandes | Baja-Media | UX | Medir en F1 con una placa real; si molesta, pasar a undo incremental |

**Regla de recorte.** Si una fase se alarga, se recortan funciones, nunca la calidad de la geometría
ni los criterios de salida.

---

## 14. Fuera de alcance

Heredado del plan original más D6, y es decisión consciente:

- Asistente IA.
- Vista 3D propia (ya existe `cam-view` + `cam-ui/view3d`).
- Sender serie, TCP y FluidNC (existen UGS, CNCjs, Candle).
- Pen plotter y funciones de nicho.
- Lectura de `.FlatPrj` (D6): la migración real es reabrir los Gerber originales.
- Dominio de madera como línea de producto activa (D4): se mantiene funcional y congelado.

---

## 15. Primer slice concreto

Si hay que empezar mañana, el orden es:

1. **`E0.7` + `E0.8`** (benchmarks + CI) — un día, y desde ese momento toda medición es comparable.
2. **`E0.5`** (corpus mínimo) — porque sin placas reales el resto se diseña a ciegas.
3. **`E0.4`** (arnés de paridad) — porque es lo que convierte E0.1 en verificable.
4. **`E0.1`** (motor de conjuntos) — el trabajo grande, ya con red.

`E0.2`, `E0.3` y `E0.6` acompañan según lo que el corpus vaya mostrando.
