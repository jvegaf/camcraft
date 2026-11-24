# Camcraft design system  (workbench v2)

Sistema visual para el rediseño de Camcraft con lenguaje de familia SolidCraft. Adapta patrones de SolidCraft (jerarquía de bordes, superficies flotantes/compactas, command search, densidad sobria) al dominio CAM de PCB de Camcraft.

**Fuentes**

| Fuente | Qué aporta |
| --- | --- |
| `crates/cam-ui/src/theme.rs` | Tokens base actuales de Camcraft (roles, paleta dark/light, métricas, contrastes probados) |
| SolidCraft UI kit / UI tour (tangency-labs/solidcraft) | Patrones de familia: border hierarchy primary/secondary/tertiary, floating/compact workspace, cards, command search, status chips, controles |
| `design/camcraft-workbench-v2/*.html` | Artboards que ejercitan este sistema |

Si un valor diverge del runtime actual de `theme.rs`, el artboard es la intención de diseño y la implementación debe converger; los contrastes AA siguen siendo obligatorios.

## Principios

1. **Canvas primero.** El viewport es la superficie más oscura/profunda; los paneles suben en la escalera de luminancia. Nada compite con la geometría.
2. **Dos señales.** `Sel` (cian) = selección/foco/estado activo. `Cut` (ámbar) = acción de corte/material/toolpath/acción primaria destructiva o de fabricación. El resto es neutro o veredicto.
3. **Jerarquía de bordes, no cajas anidadas.** Los bordes comunican estructura; no se usan para decorar cada campo.
4. **Familia SolidCraft, dominio Camcraft.** Se toman patrones de shell (floating/compact, browser/inspector/command search/status), no el modelo CAD paramétrico de SolidCraft.

## Jerarquía de bordes (SolidCraft family)

| Nivel | Rol | Uso en Camcraft | Token |
| --- | --- | --- | --- |
| **Primary** | Borde exterior | Cards flotantes, paneles docked, menús/popovers, dialogs, start screen | `BorderPrimary` |
| **Secondary** | División mayor | Header/footer de panel, separación ribbon/contenido, split entre zonas principales | `BorderSecondary` |
| **Tertiary** | Separador interno | Grupos dentro de una sección, filas, divisores suaves | `BorderTertiary` |

Mapeo a tokens actuales: `BorderPrimary` = `Border` (`#35404c` dark / `#c3ccd6` light), `BorderSecondary` = `Hairline` reforzado en contexto de sección (`#262d36` dark / `#dde3ea` light), `BorderTertiary` = uso de `Hairline` al 70% o `Hairline` simple según superficie. No agregar bordes nuevos si `Hairline`/`Border` resuelven el caso.

## Color

### Dark (default)

| Rol | Valor | Uso |
| --- | --- | --- |
| `App` | `#12151a` | Fondo de app detrás de paneles |
| `Canvas` | `#0d1014` | Viewport/dibujo, superficie más profunda |
| `Panel` | `#171b21` | Paneles, app bar, status bar, cards docked |
| `Raised` | `#1e242b` | Controles, hover, filas elevadas |
| `Overlay` | `#232a32` | Popovers, menús, dialogs, cards flotantes |
| `Hairline` | `#262d36` | Separador terciario/secundario sutil |
| `Border` | `#35404c` | Borde primario de controles/cards |
| `TextHi` | `#e7ecf3` | Títulos, valores activos, nombres |
| `Text` | `#c3ccd8` | Texto normal, labels, filas |
| `TextDim` | `#8b95a3` | Metadata, ayudas, unidades |
| `TextOff` | `#5c6673` | Disabled/placeholder; nunca cuerpo |
| `Sel` | `#6fd3e8` | Selección, foco, activo |
| `SelSoft` | `rgba(111,211,232,.13)` | Wash de selección |
| `Cut` | `#f5a623` | Toolpath/corte/acción primaria CAM |
| `CutSoft` | `rgba(245,166,35,.13)` | Wash de corte/primario |
| `Ok` | `#5ec27f` | Válido, listo |
| `Warn` | `#f0b429` | Advertencia |
| `Danger` | `#f2635f` | Error/refusal |

### Light

| Rol | Valor | Rol | Valor |
| --- | --- | --- | --- |
| `App` | `#eef1f5` | `Sel` | `#0d7c92` |
| `Canvas` | `#f7f9fb` | `SelSoft` | `rgba(13,124,146,.09)` |
| `Panel` | `#ffffff` | `Cut` | `#8b5100` |
| `Raised` | `#f2f5f8` | `CutSoft` | `rgba(139,81,0,.08)` |
| `Overlay` | `#ffffff` | `Ok` | `#1f7a43` |
| `Hairline` | `#dde3ea` | `Warn` | `#96610a` |
| `Border` | `#c3ccd6` | `Danger` | `#b3342f` |
| `TextHi` | `#12181f` | | |
| `Text` | `#2b3440` | | |
| `TextDim` | `#5f6b7a` | | |
| `TextOff` | `#8b95a3` | | |

Los colores de material/stock (`StockTop/Bottom/Edge`) se mantienen del sistema actual para la vista 3D; no se usan como acentos de UI.

## Tipografía

Familia: `Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`.
Mono: `ui-monospace, SFMono-Regular, Menlo, monospace` para coordenadas, IDs, valores numéricos tabulares.

| Estilo | Tamaño / peso | Uso |
| --- | --- | --- |
| Hero | 28 / 600 | Pantallas vacías/start, con mucha restricción |
| Dialog | 20 / 600 | Título de modal/dialog |
| Title | 15 / 600 | Nombre de documento, sección fuerte |
| Body | 13 / 400 | Texto base |
| Button/Label | 12-13 / 500 | Botones, tabs, labels de control |
| Small | 12 / 400 | Metadata, filas secundarias |
| Caption | 11 / 400-600 | Encabezados de sección, chips, estado |
| Mono | 12 / 400 | Coords, Z, unidades, IDs |

## Geometría

| Token | Valor | Uso |
| --- | --- | --- |
| `space` | 2,4,6,8,12,16,24,32 | Escala única |
| `radius-control` | 4 | Botones, inputs, chips pequeños |
| `radius-panel` | 6 | Cards docked, paneles, filas agrupadas |
| `radius-overlay` | 8 | Cards flotantes / popovers grandes |
| `radius-dialog` | 10 | Modals/dialogs |
| `control-h` | 28-30 | Controles estándar |
| `row-h` | 28-32 | Filas de lista/browser |
| `appbar-h` | 48 | Top bar compacta |
| `status-h` | 30-32 | Status bar |
| `hairline` | 1 | Separadores y bordes finos |

## Componentes (patrones SolidCraft family)

### App bar / command bar

- Fondo `Panel`, borde inferior `Hairline`.
- Izquierda: mark/producto, separador, documento/estado.
- Centro: command search con scope visible (`Buscar comando…`), atajo `⌘K`.
- Derecha: acciones de vista/layout/tema/configuración, unidades.

### Floating card

- Fondo `Overlay` con leve blur opcional (en implementación; en artboard, color sólido).
- Borde `Border` (primary), radio `radius-overlay`, sombra `0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42)`.
- Header 38px: título caption uppercase, metadata a la derecha, acciones quietas.
- Body con padding 10-12px; scroll interno si hace falta.

### Browser / árbol de proyecto

- Filas completas clickeables, altura ~28-32px.
- Selección actual con `SelSoft` + texto `TextHi`; nunca solo color.
- Íconos/puntos de estado con significado (copper/drill/cut/ok), con texto/metadato.

### Inspector contextual

- Edita el objeto seleccionado; no compite con browser.
- Campos con label `TextDim`, input `App/Raised` + borde `Border`, valor `Text`.
- Veredictos/estado como callout con borde semántico suave.

### Command search

- Siempre visible o invocable por `⌘K`.
- Placeholder dice el alcance; no promete buscar “todo” si no lo hace.

### Status bar

- Izquierda: coords/estado inmediato (mono).
- Centro/derecha: contexto, unidades, mensajes, estado de salud.
- Chips discretos; sin ruido.

### Start screen

- Misma familia visual que el shell.
- Dos acciones claras: nuevo/abrir; recientes con nombre, path, tiempo.
- No desplaza el lenguaje a “marketing hero”.

## Accesibilidad y comportamiento

- Contraste AA mínimo en texto/controles según `theme.rs` tests y WCAG.
- `:focus-visible` con `Sel`, offset claro.
- `prefers-reduced-motion`: reducir transiciones/animaciones.
- Estados vacíos, sin resultado, error, carga y deshabilitado explícitos.
- No depender de hover para acciones esenciales.
