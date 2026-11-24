# Escena fija compartida para spikes

Ambos spikes deben usar la **misma escena** para que FPS/input/viewport sean comparables.

## Escena canónica

- Stock: 100mm x 100mm x 20mm, origen coherente con camcraft.
- Geometría: un pocket rectangular simple centrado (ej. 60x40mm, profundidad 5mm) o la escena demo mínima que ya pueda producir `cam-view`/`cam-model` con menor glue.
- Cámara inicial: isométrica/3D oblicua que muestre stock completo.
- Sin dependencia de archivos externos del usuario.

## Implementación

- Preferir construir la escena usando crates existentes (`cam-view`, `cam-model`, `cam-geom`, `cam-ops`, `cam-sim` según corresponda).
- Si hace falta un exportador intermedio (JSON/mesh/bin), ponerlo dentro del spike que lo necesite y documentarlo; no tocar `cam-view` sin coordinación.
- Registrar en el REPORT: cómo se construyó la escena, formato intermedio, y checksum/cuenta de vértices/triángulos si aplica.

## Verificación mínima

- El viewport muestra el stock y la geometría.
- Orbit/pan/zoom funcionan.
- La escena es determinista (misma entrada => misma salida visual/datos).
