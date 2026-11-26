// Curated catalog of the Atomic Design components (097 T2, S2 + S4).
//
// One entry per component the generated inventory derived. The states are read from each component's
// own props interface — never invented — and the sheet renders the real instance for every state.
// The honesty rule (097 T2): a component that cannot render in isolation because it needs host IPC
// (`@tauri-apps/api`), a live store or a three.js canvas is `renderable: false`, carries a one-line
// reason, and its states are shown as data instead of pretending to be covered.
import type { Component } from 'svelte';
import Button from '$lib/components/atoms/Button.svelte';
import Icon from '$lib/components/atoms/Icon.svelte';
import IconButton from '$lib/components/atoms/IconButton.svelte';
import KeyHint from '$lib/components/atoms/KeyHint.svelte';
import StatusDot from '$lib/components/atoms/StatusDot.svelte';
import CommandSearch from '$lib/components/molecules/CommandSearch.svelte';
import FloatCard from '$lib/components/molecules/FloatCard.svelte';
import PanelRow from '$lib/components/molecules/PanelRow.svelte';
import PanelSection from '$lib/components/molecules/PanelSection.svelte';
import CamObjectBrowser from '$lib/components/organisms/CamObjectBrowser.svelte';
import OperationInspector from '$lib/components/organisms/OperationInspector.svelte';
import StatusBar from '$lib/components/organisms/StatusBar.svelte';
import ToolRail from '$lib/components/organisms/ToolRail.svelte';
import TopBar from '$lib/components/organisms/TopBar.svelte';
import ViewportHud from '$lib/components/organisms/ViewportHud.svelte';
import WorkbenchTemplate from '$lib/components/templates/WorkbenchTemplate.svelte';
import { type ComponentEntry, LAYERS, type Layer } from './inventory';

export type CatalogComponent = Component<Record<string, unknown>>;

const asCatalogComponent = (value: unknown): CatalogComponent =>
  value as CatalogComponent;

export type CatalogState = {
  /** Short name of the variant the state exercises, shown on the sheet. */
  label: string;
  /** Props handed verbatim to the real component. */
  props: Record<string, unknown>;
  /** Default slot content, for the components whose `children` is a snippet. */
  content?: string;
};

export type CatalogEntry = {
  slug: string;
  name: string;
  layer: Layer;
  /** One-line purpose. */
  purpose: string;
  /** Repository-relative component path, the key the inventory cross-references. */
  path: string;
  component: CatalogComponent;
  renderable: boolean;
  /** Why an entry cannot render in isolation. Required when `renderable` is false. */
  reason?: string;
  states: CatalogState[];
};

const noop = () => undefined;
const noopQuery = (_query: string) => undefined;

export const CATALOG: readonly CatalogEntry[] = [
  {
    slug: 'button',
    name: 'Button',
    layer: 'atom',
    purpose: 'Botón etiquetado con las variantes de acento y los dos tamaños.',
    path: 'src/lib/components/atoms/Button.svelte',
    component: asCatalogComponent(Button),
    renderable: true,
    states: [
      {
        label: 'primary',
        props: { variant: 'primary' },
        content: 'Generar plan'
      },
      {
        label: 'secondary',
        props: { variant: 'secondary' },
        content: 'Importar'
      },
      { label: 'ghost', props: { variant: 'ghost' }, content: 'Cancelar' },
      {
        label: 'sm',
        props: { variant: 'secondary', size: 'sm' },
        content: 'Guardar'
      },
      {
        label: 'md',
        props: { variant: 'secondary', size: 'md' },
        content: 'Guardar'
      },
      {
        label: 'disabled',
        props: { variant: 'secondary', disabled: true },
        content: 'Capas'
      }
    ]
  },
  {
    slug: 'icon',
    name: 'Icon',
    layer: 'atom',
    purpose:
      'Dibujo 24×24 del set de iconos; decorativo salvo que reciba label.',
    path: 'src/lib/components/atoms/Icon.svelte',
    component: asCatalogComponent(Icon),
    renderable: true,
    states: [
      { label: 'sm', props: { name: 'search', size: 'sm' } },
      { label: 'md', props: { name: 'search' } },
      { label: 'lg', props: { name: 'search', size: 'lg' } },
      {
        label: 'con label',
        props: { name: 'settings', label: 'Configuración' }
      }
    ]
  },
  {
    slug: 'icon-button',
    name: 'IconButton',
    layer: 'atom',
    purpose: 'Control cuadrado solo-icono; el nombre accesible es obligatorio.',
    path: 'src/lib/components/atoms/IconButton.svelte',
    component: asCatalogComponent(IconButton),
    renderable: true,
    states: [
      { label: 'md', props: { ariaLabel: 'Abrir', icon: 'folder' } },
      {
        label: 'sm',
        props: { ariaLabel: 'Guardar', icon: 'save', size: 'sm' }
      },
      {
        label: 'disabled',
        props: { ariaLabel: 'Capas', icon: 'layers', disabled: true }
      }
    ]
  },
  {
    slug: 'key-hint',
    name: 'KeyHint',
    layer: 'atom',
    purpose: 'Chip de atajo de teclado del chrome de la app.',
    path: 'src/lib/components/atoms/KeyHint.svelte',
    component: asCatalogComponent(KeyHint),
    renderable: true,
    states: [
      { label: 'por defecto', props: {} },
      { label: 'personalizado', props: { keys: 'Esc' } }
    ]
  },
  {
    slug: 'status-dot',
    name: 'StatusDot',
    layer: 'atom',
    purpose: 'Punto tonal; sin label es decorativo y no comunica por color.',
    path: 'src/lib/components/atoms/StatusDot.svelte',
    component: asCatalogComponent(StatusDot),
    renderable: true,
    states: [
      { label: 'neutral', props: { tone: 'neutral' } },
      { label: 'ok', props: { tone: 'ok' } },
      { label: 'selected', props: { tone: 'selected' } },
      { label: 'draft', props: { tone: 'draft' } },
      { label: 'cut', props: { tone: 'cut' } },
      { label: 'warn', props: { tone: 'warn' } },
      { label: 'danger', props: { tone: 'danger' } },
      { label: 'con label', props: { tone: 'ok', label: 'Estado: válido' } }
    ]
  },
  {
    slug: 'command-search',
    name: 'CommandSearch',
    layer: 'molecule',
    purpose:
      'Campo de búsqueda del app bar; sin onquery queda deshabilitado y dice por qué.',
    path: 'src/lib/components/molecules/CommandSearch.svelte',
    component: asCatalogComponent(CommandSearch),
    renderable: true,
    states: [
      { label: 'sin handler', props: {} },
      {
        label: 'enabled',
        props: { onquery: noopQuery, placeholder: 'Buscar comando…' }
      }
    ]
  },
  {
    slug: 'float-card',
    name: 'FloatCard',
    layer: 'molecule',
    purpose: 'Superficie flotante con cabecera y cuerpo desplazable.',
    path: 'src/lib/components/molecules/FloatCard.svelte',
    component: asCatalogComponent(FloatCard),
    renderable: true,
    states: [
      {
        label: 'sin meta',
        props: { title: 'Inspector' },
        content: 'Contenido de la tarjeta.'
      },
      {
        label: 'con meta',
        props: { title: 'Browser', meta: '3 capas' },
        content: 'Contenido de la tarjeta.'
      }
    ]
  },
  {
    slug: 'panel-row',
    name: 'PanelRow',
    layer: 'molecule',
    purpose: 'Fila de panel con punto o icono, etiqueta y meta.',
    path: 'src/lib/components/molecules/PanelRow.svelte',
    component: asCatalogComponent(PanelRow),
    renderable: true,
    states: [
      { label: 'etiqueta', props: { label: 'Proyecto', meta: 'board.cmc' } },
      {
        label: 'con punto',
        props: {
          label: 'Board',
          meta: '2 capas',
          dot: 'ok',
          dotLabel: 'Estado: válido'
        }
      },
      {
        label: 'con icono',
        props: { label: 'Operaciones', meta: '1', icon: 'layers' }
      },
      {
        label: 'selected',
        props: {
          label: 'Tamaño',
          meta: '120×80 mm',
          selected: true,
          onclick: noop
        }
      }
    ]
  },
  {
    slug: 'panel-section',
    name: 'PanelSection',
    layer: 'molecule',
    purpose:
      'Bloque de panel con cabecera, conteo y cuerpo, opcionalmente colapsable.',
    path: 'src/lib/components/molecules/PanelSection.svelte',
    component: asCatalogComponent(PanelSection),
    renderable: true,
    states: [
      {
        label: 'con conteo',
        props: { title: 'Board', count: 2 },
        content: 'Filas de la sección.'
      },
      {
        label: 'collapsible',
        props: {
          title: 'Setup',
          collapsible: true,
          collapsed: false,
          ontoggle: noop
        },
        content: 'Filas de la sección.'
      },
      {
        label: 'collapsed',
        props: {
          title: 'Salida',
          collapsible: true,
          collapsed: true,
          ontoggle: noop
        },
        content: 'Filas de la sección.'
      }
    ]
  },
  {
    slug: 'cam-object-browser',
    name: 'CamObjectBrowser',
    layer: 'organism',
    purpose: 'Browser del workbench organizado por objeto CAM.',
    path: 'src/lib/components/organisms/CamObjectBrowser.svelte',
    component: asCatalogComponent(CamObjectBrowser),
    renderable: false,
    reason:
      'Lee el store vivo de workbench y el documento abierto; no renderiza aislado.',
    states: [
      { label: 'sin handler', props: {} },
      { label: 'con onOpenRecent', props: { onOpenRecent: noop } }
    ]
  },
  {
    slug: 'operation-inspector',
    name: 'OperationInspector',
    layer: 'organism',
    purpose:
      'Inspector de la operación seleccionada y del estado del workbench.',
    path: 'src/lib/components/organisms/OperationInspector.svelte',
    component: asCatalogComponent(OperationInspector),
    renderable: false,
    reason: 'Lee el store vivo de workbench; no renderiza aislado.',
    states: [{ label: 'estado del store', props: {} }]
  },
  {
    slug: 'status-bar',
    name: 'StatusBar',
    layer: 'organism',
    purpose:
      'Barra de estado con coordenadas, selección y salud del workbench.',
    path: 'src/lib/components/organisms/StatusBar.svelte',
    component: asCatalogComponent(StatusBar),
    renderable: false,
    reason:
      'Lee el puntero, la selección y el estado vivos del workbench; no renderiza aislado.',
    states: [{ label: 'estado del store', props: {} }]
  },
  {
    slug: 'tool-rail',
    name: 'ToolRail',
    layer: 'organism',
    purpose: 'Rail de acciones rápidas del viewport.',
    path: 'src/lib/components/organisms/ToolRail.svelte',
    component: asCatalogComponent(ToolRail),
    renderable: true,
    states: [
      { label: 'sin handler', props: {} },
      { label: 'con onOpen', props: { onOpen: noop } }
    ]
  },
  {
    slug: 'top-bar',
    name: 'TopBar',
    layer: 'organism',
    purpose: 'App bar con marca, proyecto, búsqueda y acciones de archivo.',
    path: 'src/lib/components/organisms/TopBar.svelte',
    component: asCatalogComponent(TopBar),
    renderable: false,
    reason:
      'La acción de tema llama a update_settings por IPC de Tauri y lee el store vivo; no renderiza aislado.',
    states: [{ label: 'sin handlers', props: {} }]
  },
  {
    slug: 'viewport-hud',
    name: 'ViewportHud',
    layer: 'organism',
    purpose: 'Tarjetas del HUD del viewport con la selección real.',
    path: 'src/lib/components/organisms/ViewportHud.svelte',
    component: asCatalogComponent(ViewportHud),
    renderable: false,
    reason: 'Lee la selección viva del workbench; no renderiza aislado.',
    states: [{ label: 'estado del store', props: {} }]
  },
  {
    slug: 'workbench-template',
    name: 'WorkbenchTemplate',
    layer: 'template',
    purpose: 'Marco de la app con una región por slot del artboard.',
    path: 'src/lib/components/templates/WorkbenchTemplate.svelte',
    component: asCatalogComponent(WorkbenchTemplate),
    renderable: true,
    states: [
      { label: 'regiones vacías', props: {} },
      { label: 'con ariaLabel', props: { ariaLabel: 'Camcraft' } }
    ]
  }
];

export const CATALOG_PATHS: ReadonlySet<string> = new Set(
  CATALOG.map(entry => entry.path)
);

const BY_SLUG = new Map(CATALOG.map(entry => [entry.slug, entry]));

export function findCatalogEntry(slug: string): CatalogEntry | undefined {
  return BY_SLUG.get(slug);
}

export type LayerSummary = {
  layer: Layer;
  total: number;
  withSheet: number;
  withTest: number;
  inUse: number;
  renderable: number;
  notRenderable: number;
};

export type BoardSummary = {
  layers: LayerSummary[];
  unclassified: ComponentEntry[];
};

/**
 * The progress board: totals and the three coverage facts per layer, derived from the inventory
 * and the manifest. A component with no manifest entry still counts in `total`; it just does not
 * count as `withSheet` nor in the `renderable` / `notRenderable` buckets.
 */
export function buildBoard(
  inventory: readonly ComponentEntry[],
  catalog: readonly CatalogEntry[]
): BoardSummary {
  const catalogByPath = new Map(catalog.map(entry => [entry.path, entry]));

  const layers = LAYERS.map(layer => {
    const components = inventory.filter(component => component.layer === layer);
    const summaries = components.map(component =>
      catalogByPath.get(component.path)
    );
    return {
      layer,
      total: components.length,
      withSheet: summaries.filter(summary => summary !== undefined).length,
      withTest: components.filter(component => component.test !== null).length,
      inUse: components.filter(component => component.consumers.length > 0)
        .length,
      renderable: summaries.filter(summary => summary?.renderable === true)
        .length,
      notRenderable: summaries.filter(summary => summary?.renderable === false)
        .length
    };
  });

  return {
    layers,
    unclassified: inventory
      .filter(component => component.layer === 'unclassified')
      .sort((left, right) => left.path.localeCompare(right.path))
  };
}
