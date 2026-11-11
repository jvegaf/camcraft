# `odd/` — cómo se lee y cómo se escribe

Cada feature del proyecto vive en un documento propio bajo `odd/tasks/`: qué se pidió, con qué specs, qué
tareas lo entregan y qué pasó. Esta carpeta no es un diario ni un plan de producto; el plan de product está
en [`../docs/ROADMAP_CAMCRAFT.md`](../docs/ROADMAP_CAMCRAFT.md) y la arquitectura en
[`../docs/SYSTEM_DESIGN_CAMCRAFT.md`](../docs/SYSTEM_DESIGN_CAMCRAFT.md).

## Por dónde se entra

**[`tasks/INDEX.md`](tasks/INDEX.md)**. Una línea por tarea, en el orden en que entraron al proyecto, con el
estado que declara su propio documento, la fecha del primer commit y el último commit que la tocó. Si tenés
que abrir un solo archivo de `odd/`, es ése.

## El nombre de las tareas

```
odd/tasks/038-offset-exact.md
          └┬┘ └────┬────┘
           │       └── el slug: qué es, en kebab-case, sin cambiar nunca
           └────────── el número: cuándo entró, para que `ls` cuente la historia
```

- **Tres dígitos, desde `001`.** Se asignan en orden de **primer commit**, no por milestone ni por carpeta:
  el orden alfabético de antes ponía `camui-shell.md` antes que `m0-foundation.md`, y no decía nada.
- **El número no cambia nunca.** Una tarea nueva toma el siguiente libre. Nada se renumera, así que un
  número que leíste en un commit, en un review o en un comentario de código sigue apuntando a lo mismo
  dentro de un año.
- **Desempate**: dos tareas del mismo día se ordenan por el nombre anterior, en orden alfabético, que es lo
  que dejó `m0`, `m1`, `m2`… en el orden en que efectivamente se hicieron.
- **El slug es estable**, así que buscar `offset-exact` sigue funcionando con o sin número.

Los documentos más viejos que el esquema usan `NNN-` desde el principio; no hay dos convenciones.

## Qué tiene adentro un documento de tarea

Cuatro bloques, siempre en este orden:

1. **Encabezado** de dos o tres líneas: qué es y para qué.
2. **`## Specs`** — lo que el owner pidió, numerado `S1`, `S2`… Las citas van **verbatim**; los requisitos
   no se resumen ni se amplían. Si un requisito cambia, se agrega la entrada al Log con la cita nueva y se
   reescribe sólo el `S#` afectado.
3. **`## Tasks`** — una línea por tarea: ID, specs que cubre, ruta (inline, worker) y el commit que la
   sostiene.
4. **`## Log`** — `L1` es el pedido original, verbatim. Después, correcciones del owner, evidencia medida y
   decisiones. Es lo último del archivo a propósito: quien lee para trabajar lee hasta ahí y no necesita
   el resto.

## El resto de la carpeta

- **`roadmap/`** — la historia del producto anterior (2.5D, M0–M15), partida por milestone. Se entra por
  [`roadmap/index.md`](roadmap/index.md). No es el plan vigente.
- **`archive/pre-pivot.md`** — la definición de producto, el stack y la arquitectura de antes del pivot,
  guardados sólo como historia.
- **`reports/`** — mediciones que sobreviven a la sesión que las hizo.

## Cómo se empieza una tarea nueva

1. Tomá el número siguiente al más alto de `tasks/`.
2. Escribí el documento con los cuatro bloques, con `L1` citando el pedido tal cual.
3. Sumá su fila a `tasks/INDEX.md`.
4. Si la tarea toca un archivo del repo, su ruta va en `## Tasks`; el commit se anota cuando existe.
