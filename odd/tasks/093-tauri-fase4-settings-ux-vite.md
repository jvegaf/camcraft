# 093 — Fase 4: settings UX y fix Vite styled-system

Cerrar settings al aplicar y permitir servir styled-system en Vite.

## Surfaces

- `src/lib/components/settings/SettingsDialog.svelte`
- `vite.config.js`
- `odd/tasks/093-tauri-fase4-settings-ux-vite.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «vale, pero un par de cosas, por un lado, si le damos al boton de aplicar. deberia de cerrarse esa ventana, no que ahora solamente se cierra en cancelar o en el boton de cerrar de la ventana. es UX no nos lo va a permitir. y por otro lado corrije esto»
- **S2 (pedido original, verbatim).** «[404] GET /styled-system/css/index.js
The request id "/home/th3g3ntl3man/Code/camcraft/styled-system/css/index.js" is outside of Vite serving allow list.

- /home/th3g3ntl3man/Code/camcraft/src/lib
- /home/th3g3ntl3man/Code/camcraft/src/routes
- /home/th3g3ntl3man/Code/camcraft/src
- /home/th3g3ntl3man/Code/camcraft/.svelte-kit
- /home/th3g3ntl3man/Code/camcraft/node_modules/.pnpm/@sveltejs+kit@2.70.4_@sveltejs+vite-plugin-svelte@7.3.1_svelte@5.59.1_vite@8.3.8__svelt_126e972a89f0642e5987ac5116aa5610/node_modules/@sveltejs/kit/src/runtime
- /home/th3g3ntl3man/Code/camcraft/node_modules

Refer to docs https://vite.dev/config/server-options.html#server-fs-allow for configurations and more details.»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Cerrar SettingsDialog al aplicar | S1 | inline | `946f600` |
| T2 | Permitir styled-system en Vite fs.allow | S2 | inline | `946f600` |
| T3 | Verificar, RDD, ODD/Engram, commit y push | S1, S2 | inline | `946f600` |

## Log

- **L1 (pedido original, verbatim).** «vale, pero un par de cosas, por un lado, si le damos al boton de aplicar. deberia de cerrarse esa ventana, no que ahora solamente se cierra en cancelar o en el boton de cerrar de la ventana. es UX no nos lo va a permitir. y por otro lado corrije esto»
- **L2 (pedido original, verbatim).** «[404] GET /styled-system/css/index.js
The request id "/home/th3g3ntl3man/Code/camcraft/styled-system/css/index.js" is outside of Vite serving allow list.

- /home/th3g3ntl3man/Code/camcraft/src/lib
- /home/th3g3ntl3man/Code/camcraft/src/routes
- /home/th3g3ntl3man/Code/camcraft/src
- /home/th3g3ntl3man/Code/camcraft/.svelte-kit
- /home/th3g3ntl3man/Code/camcraft/node_modules/.pnpm/@sveltejs+kit@2.70.4_@sveltejs+vite-plugin-svelte@7.3.1_svelte@5.59.1_vite@8.3.8__svelt_126e972a89f0642e5987ac5116aa5610/node_modules/@sveltejs/kit/src/runtime
- /home/th3g3ntl3man/Code/camcraft/node_modules

Refer to docs https://vite.dev/config/server-options.html#server-fs-allow for configurations and more details.»
- **L3 (ejecución, 2026-11-24).** Aplicar cierra el diálogo tras guardar; `vite.config.js` agrega `server.fs.allow` para `./styled-system`. `pnpm check` OK.
- **L4 (cierre, 2026-11-24).** Commit `946f600`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
