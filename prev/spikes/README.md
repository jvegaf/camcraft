# Spikes: Tauri 2 vs GPUI

Dos spikes en paralelo para evaluar la migración del chrome/DS de camcraft. Cada spike corre en su propio worktree y no toca `crates/cam-ui`.

## Tareas

- `077-spike-tauri-webview` → `spikes/tauri/`, worktree propio.
- `078-spike-gpui-native` → `spikes/gpui/`, worktree propio.

## Regla de no-solape

- Código nuevo solo dentro del spike propio (`spikes/tauri/**` o `spikes/gpui/**`).
- `spikes/shared/**` es solo lectura para ambos agentes durante el spike.
- No tocar `crates/cam-ui`.
- No tocar `crates/cam-view` salvo cambio mínimo coordinado, declarado antes y con claim explícito; preferir exportar/consumir desde el spike.
- `design/` y `fixtures/ui/` son solo lectura.

## Slice vertical común (obligatoria)

Ambos spikes deben implementar lo mismo:

1. Ventana desktop con theme dark/light usando tokens del DS.
2. Start screen mínima: marca, New/Open, lista reciente mock o real.
3. Viewport con una escena fija exportada por `cam-view` (misma escena, definida en `spikes/shared/SCENE.md`).
4. Cámara básica: orbit/pan/zoom.
5. Un overlay UI sobre/junto al viewport (status o panel) para probar composición.

## Scorecard común

Completar en `spikes/tauri/REPORT.md` y `spikes/gpui/REPORT.md`.

| Métrica | Cómo medir | Señal |
|---|---|---|
| Fidelidad DS | Checklist visual contra `design/`: tipografía, spacing, radios, bordes, colores, estados | Tauri alto; GPUI aceptable sin CSS |
| Viewport FPS | Escena fija, 60s, orbit continuo; reportar avg/min/1% low | >= 60 avg, sin stutters visibles |
| Input latency | Orbit/pan/zoom/click; reportar percepción y método | Sin lag perceptible |
| Integración `cam-view` | Describir forma: directo Rust, serialización, adaptación | Menos glue = mejor |
| Complejidad de build | Pasos desde cero, deps extra, tiempo en Linux | Reproducible en <15 min |
| Tamaño/packaging | Binario/appimage/deb aproximado | Informativo |
| Riesgo técnico | Bloqueos, hacks, APIs inestables | Alto/medio/bajo |
| Costo de migración estimado | Por crate: reusar/rewrite/bridge | Semanas/persona grueso |

## Cierre

1. Cada agente entrega su `REPORT.md` con capturas/gif y comandos reproducibles.
2. Revisión conjunta: correr ambos spikes en la misma máquina, misma escena.
3. Decisión: Tauri / GPUI / no migrar aún.
