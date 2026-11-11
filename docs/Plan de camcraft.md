# Plan de camcraft

Plan por fases para construir camcraft, un CAM de PCB open source en Rust, sobre el motor de campilot y con la lógica heredada de FlatCAM-plus.

El plan asume que camcraft empieza como un conjunto de crates nuevos dentro del workspace de campilot, y que se separa o renombra cuando la identidad del producto esté clara. Los plazos son orientativos y dependen mucho del ritmo de trabajo; lo importante es el orden y los criterios de salida de cada fase.

## Fase 1: Importadores (Gerber y Excellon)

- **`cam-gerber`**
  - Parser RS-274X: aperturas estándar (C, R, O, P) y macros de apertura.
  - Polaridad clear/dark, step & repeat, regiones (G36/G37), interpolación circular, unidades y formato de coordenadas.
  - Primero evaluar los crates existentes (`gerber-types`, `gerber_parser`) y decidir si usarlos o escribir el propio.
- **Conversión a geometría:** cada flash y trazo se convierte a regiones, y las capas con polaridad se resuelven con la librería de booleanas elegida en la fase 0.
- **`cam-excellon`:** parser de herramientas, coordenadas, unidades, ceros omitidos y slots. Producir un modelo de taladros compatible con la estrategia de drilling.
- **Informe de importación:** reutilizar el patrón de `cam-import` (convertido, omitido con motivo y ubicación). Nada debe descartarse en silencio.
- **Tests:** comparar con las geometrías exportadas de FlatCAM. Añadir un corpus de Gerber de varias herramientas (KiCad, EasyEDA, Eagle, Altium), porque cada una tiene sus rarezas.

**Salida:** abrir los Gerber y Excellon de las placas de referencia y obtener geometría idéntica, dentro de tolerancia, a la de FlatCAM.

## Fase 2: Modelo de datos PCB

- Ampliar `cam-model` con objetos Gerber, Excellon y capas de placa (cobre superior e inferior, máscara, serigrafía, contorno).
- Gestionar espejado de la cara inferior, referencia de origen y orientación.
- Subir la versión del schema (`c2d/document@3`) con su migración, como ya se hace con la @1 a la @2.
- Ampliar la librería de herramientas SQLite: V-bits con ángulo y punta, fresas de aislamiento, brocas, y perfiles equivalentes a la Tools DB.
- Mantener la compatibilidad hacia atrás de los documentos existentes.

**Salida:** un documento `.c2d` que guarda y recupera una placa completa con sus capas.

## Fase 3: Operaciones PCB (el núcleo)

Orden recomendado, de menos a más complejo, y cada una con su oráculo de FlatCAM:

1. **Drilling PCB:** reutiliza el drill-bank y las estrategias de peck. Falta la selección por diámetro y el reagrupado por herramienta.
2. **Cutout:** perfil exterior con tabs y puentes. Reutiliza el perfil con tabs que ya existe.
3. **Isolation**
   - Unión del cobre, offset hacia fuera y multipasada.
   - Profundidad derivada del V-bit.
   - Opción de conservar pours (lo que hacía "Generate With Copper", reimplementado desde cero).
4. **NCC (Non-Copper Clearing)**
   - Es la operación crítica en cuanto a rendimiento.
   - Diseño pensado para velocidad: paralelismo por isla con `rayon`, índice espacial (`rstar`), offsets incrementales y rest machining entre herramientas.
   - Métodos de relleno equivalentes a los de FlatCAM (standard, seed y lines).
5. **Paint:** relleno de áreas arbitrarias, derivado del pocket con islas.
6. **Panelize y otras utilidades:** matrices de placas, film y punch, según el valor para el proyecto.

Para cada operación se define un criterio de aceptación: la geometría de camcraft cubre el mismo cobre y no introduce gouge, medido con `cam-sim`.

**Salida:** isolation, NCC, drill y cutout generan G-code válido en las placas de referencia, frente a la línea base de FlatCAM.

## Fase 4: Post-procesadores y G-code

- Portar los preprocesadores básicos de FlatCAM (GRBL, y los de Marlin o similares que se usen) al dialecto `.pp`. Son de solo datos, así que es un trabajo simple.
- Verificar G-code con tests golden como ya se hace.
- **Autolevelling:** implementarlo como transformación del move stream (compensación Z con mapa de alturas, segmentando arcos y movimientos largos). Encaja de forma natural con la arquitectura.
- Compensación de cama (offset y skew), si resulta de interés.

**Salida:** G-code listo para máquina con compensación Z opcional.

## Fase 5: Interfaz de usuario

Es la fase con más trabajo y la que más tiempo suele subestimarse.

- Vista de capas Gerber con color por capa, visibilidad y modo cobre.
- Árbol de proyecto (placa, capas, operaciones).
- Formularios de toolpath, que hoy faltan incluso para las estrategias actuales de campilot: isolation, NCC, drill, cutout y paint, más el de generar G-code.
- Panel de herramientas ligado a la librería SQLite.
- Previsualización de toolpaths y simulación integradas (ya existen `cam-sim` y la vista 3D).
- Atajos de teclado y guardado con Ctrl+S, que hoy faltan.

**Salida:** un flujo completo en la interfaz sin tocar `cam-cli`: importar, configurar, generar, simular y exportar.

## Fase 6: Endurecimiento y publicación

- Corpus amplio de placas reales y tests de regresión de geometría.
- Benchmarks reproducibles y publicados, integrados en CI.
- Empaquetado para Windows y Linux, y documentación (formato, CLI, guía de migración desde FlatCAM).
- Preparar el repositorio: README, guía de contribución, issues etiquetados y una versión alfa para probadores.

## Aplazado deliberadamente

Estas funciones del FlatCAM Plus actual no entran en el alcance, y es una decisión consciente:

- Asistente IA (descartado).
- Vista 3D propia (ya la tiene campilot).
- Sender serie, TCP y FluidNC. Si se quiere más adelante, será una fase aparte; mientras tanto existen UGS, CNCjs y Candle.
- Pen plotter y otras funciones de nicho, a valorar según demanda.

## Riesgos a vigilar durante todo el proyecto

- **Robustez de booleanas con Gerber reales:** es la fuente de errores más frecuente. Un corpus grande de placas desde la fase 1 es la mejor defensa.
- **Rendimiento del offset exacto:** si la prueba de distancia contra todos los contornos penaliza, hacerla opcional (modo estricto y tests).
- **Alcance:** si una fase se alarga, recortar funciones, no calidad.

## Hitos de verificación

1. Importar el corpus de placas.
2. Primer G-code de isolation y drill validado en simulación (mitad de fase 3).
3. NCC más rápida que la de FlatCAM en la placa grande (fin de fase 3).
4. Flujo completo en la interfaz (fin de fase 5).
