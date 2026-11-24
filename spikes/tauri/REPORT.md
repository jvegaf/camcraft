# REPORT — Spike Tauri 2 + webview

## Resumen

- Estado: pendiente
- Fecha:
- Agente:
- Commit/branch:

## Cómo reproducir

```bash
# deps
# comandos
```

## Slice vertical

- [ ] Ventana desktop dark/light con tokens DS
- [ ] Start screen mínima (marca, New/Open, recents mock/real)
- [ ] Viewport con escena fija `spikes/shared/SCENE.md`
- [ ] Cámara orbit/pan/zoom
- [ ] Overlay UI sobre/junto al viewport

## Scorecard

| Métrica | Resultado | Método | Evidencia | Riesgo |
|---|---|---|---:|---|
| Fidelidad DS |  | checklist vs `design/` |  |  |
| Viewport FPS |  | 60s orbit continuo, avg/min/1% low |  |  |
| Input latency |  | orbit/pan/zoom/click |  |  |
| Integración `cam-view` |  | directo/serialización/adaptación |  |  |
| Complejidad de build |  | pasos desde cero en Linux |  |  |
| Tamaño/packaging |  | binario/appimage/deb |  |  |
| Riesgo técnico |  | bloqueos/hacks/APIs inestables |  |  |
| Costo migración estimado |  | por crate: reusar/rewrite/bridge |  |  |

## Hallazgos

### Funciona bien

- 

### Bloqueos / riesgos

- 

### Costo de migración por crate

| Crate | Acción | Notas |
|---|---|---|
| `cam-ui` |  |  |
| `cam-view` |  |  |
| `cam-desktop` |  |  |
| `cam-app` |  |  |
| `cam-ops` |  |  |
| `cam-model` |  |  |

## Evidencia

- Capturas/gif:
- Logs/perf:
