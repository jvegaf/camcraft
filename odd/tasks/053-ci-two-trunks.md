# 053 — CI y flujo de dos troncos

Arreglar el CI roto, bajar sus minutos por push y definir el flujo de ramas: `dev` integra, `master` publica releases por PR con tag, con hooks locales que evitan gastar minutos en lo que se puede verificar antes.

## Surfaces

- `.github/workflows/`
- `.githooks/`
- `scripts/check.sh`
- `scripts/parity/`
- `fixtures/corpus/`
- `justfile`
- `AGENTS.md`
- `odd/roadmap/`

## Specs

- **S1 (pedido, verbatim).** «comprueba porque a cascado el CI en la ultima subida y arreglalo.»
- **S2 (pedido, verbatim).** «tenemos que darle una vueltecita al CI, por un lado tenemos que subir la version del paso de checkout, que creo que va por la 7, ademas necesitamos optimizarlo para tener maxima eficiencia, no podemos tirar tantos minutos de ejecucion cada vez que hagamos push, quizas poniendo hooks pre-commit o pre-push o ambos...»
- **S3 (pedido, verbatim).** «que te parece si ya de paso le damos una vuelta de rosca mas al asunto y definimos un git workflow? por ejemplo podemos volver a tener rama dev en la cual iria el A, que gasta menos minutos, y los releases los hariamos en master, teniendo que hacer PR desde dev contra master, en la cual ya podriamos poner el B»
- **S4 (decisión, verbatim).** «Master va a ser solamente para las releases, y va a ser meditante pr con tag. Así que dev sera para desarrollo. de momento no vamos a tener nightlies. Intenta proteger master como has indicado»
- **S5 (criterio de aceptación).** El CI no falla en `master`; un push a `dev` cuesta menos minutos que hoy y no pierde ninguna medición del corpus; `master` no acepta pushes directos y exige `gate`, `parity` y `bench`; los hooks locales existen y son instalables con una receta.

## Tasks

| ID  | Tarea                                                                                                    | Specs  | Ruta   | Commit               |
| --- | -------------------------------------------------------------------------------------------------------- | ------ | ------ | -------------------- |
| T1  | Arreglar el job de paridad que no compilaba (medidor a `TolerancePolicy`) y hacer que el gate lo compile | S1, S5 | inline | `3f1abe1`            |
| T2  | Re-pinnear los goldens del corpus tras la ola W2                                                         | S1     | inline | `3f1abe1`            |
| T3  | Alinear los dos tests que la migración W3 dejó inconsistentes                                            | S1     | inline | `0be4139`, `9fd786c` |
| T4  | Arreglar el test flaky que rompía el gate sólo en CI (`Scratch` por proceso)                             | S1     | inline | `32d6f01`            |
| T5  | Crear `dev`, proteger `master` con PR + checks requeridos                                                | S3, S4 | inline | (settings del repo)  |
| T6  | Reescribir `ci.yml`: A en `dev`, B en PR a `master`, acciones v7, caché compartida                       | S2, S3 | inline | `307ab4d`            |
| T7  | Bajar el tope del arnés y re-pinnear (30 s → 20 s, medido)                                               | S2, S5 | inline | `307ab4d`            |
| T8  | Hooks `pre-commit` / `pre-push` + `just hooks`                                                           | S2     | inline | `307ab4d`            |
| T9  | Documentar el flujo (`workflow.md`, `AGENTS.md`, `waves.md`, filas E0.4/E0.8)                            | S3, S4 | inline | `307ab4d`            |

## Log

- **L1 (pedido, verbatim).** «comprueba porque a cascado el CI en la ultima subida y arreglalo.»
- **L2 (diagnóstico).** Dos causas distintas. (a) El job `parity` **no compilaba**: la ola W3 cambió `image()` y `tessellate()` de `f64` a `TolerancePolicy` y nadie actualizó `scripts/parity/measure/src/main.rs`, que vive **fuera del workspace** — `cargo clippy/test --workspace` nunca lo toca, así que el único que lo vio fue el job de CI. (b) Los **goldens estaban viejos**: 37 divergencias, todas `refused → parsed`, porque W2a/W2b enseñaron a los parsers lo que los goldens pinneaban como rechazo. Además el gate fallaba por un test flaky (`preview::the_v_bit_is_drawn…`) que localmente pasaba.
- **L3 (por qué el flaky sólo caía en CI).** `Scratch::new(name)` armaba la carpeta con **pid + nombre**, y todos los tests de un binario comparten el pid: dos tests en paralelo se borraban y recreaban la misma ruta, así que `open_library_at` fallaba. Con más núcleos libres (CI) la carrera se dispara; en local no. Arreglado con un contador atómico por llamada (`32d6f01`), verificado con `--test-threads=16` tres veces (55/55).
- **L4 (verificación del arreglo del CI).** `parity` re-pinneado y **confirmado con una segunda corrida completa**: «the corpus matches its goldens». `just check` completo en verde, con el paso nuevo `== parity harness ==` que compila el medidor: esa rotura ya no puede llegar a CI sin verse antes en local.
- **L5 (medición del tope del arnés, y un error propio).** 30 s → 10m26 con 12 artefactos medidos; 10 s → 4m08 pero **10** medidos, porque dos capas de `substitute-easyeda` (mask-top, mask-bottom) sí terminan entre 10 y 30 s. Mi primera estimación («ninguno termina cerca del tope») salió de datos agregados y era falsa para esas dos. Elegido **20 s**: 7m26 y los 12 medidos. Los 18 que no terminan son deuda de performance del kernel, no del arnés.
- **L6 (dos troncos).** `dev` creado desde `master` y publicado. `master` protegido por API: PR obligatorio, checks requeridos `gate`/`parity`/`bench`, `strict`, admins incluidos, sin force-push ni borrado, conversación resuelta. Los nombres de job del workflow **son** esos contextos: renombrar un job lo desrequiere en silencio.
- **L7 (`ci.yml`).** `push` a `dev`: `gate` siempre + `parity` sólo si el diff toca `fixtures/corpus/`, `crates/cam-{gerber,excellon,geom,import}/` o `scripts/parity/`; sin `bench`. `pull_request` a `master`: los tres siempre. `workflow_dispatch` para corrida completa a mano. `checkout@v7`, `upload-artifact@v7`, `rust-cache` con `shared-key` y `save-if` sólo en `dev`. El paso que decide si medir **reporta success cuando saltea**, porque es un contexto requerido y un contexto que no reporta bloquea el release.
- **L8 (hooks).** `.githooks/pre-commit` (fmt si hay Rust staged, links de docs si hay markdown, build del medidor si se tocó el arnés o un parser) y `.githooks/pre-push` (clippy + tests si el rango pusheado toca Rust; `parity` si toca lo que el corpus mide). El rango se lee del remoto, no del árbol de trabajo, y cada paso salteado **dice por qué**: un hook que verifica en silencio es peor que no tener hook. `just hooks` / `just hooks-off` para instalarlos.
- **L9 (review nativo).** Linaje `review-fb2f278b8fe32cd3` (lentes `risk` + `resilience`, tier high, 16 paths / 319 líneas): **aprobado**, autoridad quemada. El refutador **descartó** el único hallazgo crítico (`ci.yml:15`) y dejó cuatro informativos (`ci.yml:100`, `ci.yml:102`, `.githooks/pre-push:50-66`, `collect.py:51`).
- **L10 (estado verificado).** `master` protegido y sin pushes directos; `dev` con `307ab4d`; `just check` verde en `dev`; hooks probados en un clon de prueba (un push que sólo toca docs saltea clippy/tests/parity y sale 0).
