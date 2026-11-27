// Curated catalog of the Atomic Design components (097 T2, S2 + S4).
//
// One entry per component the generated inventory derived. The states are read from each component's
// own props interface — never invented — and the sheet renders the real instance for every state.
// The honesty rule (097 T2): a component that cannot render in isolation because it needs host IPC
// (`@tauri-apps/api`), a live store or a three.js canvas is `renderable: false`, carries a one-line
// reason, and its states are shown as data instead of pretending to be covered.
import type { Component } from 'svelte';
import Button from '$lib/components/atoms/Button.svelte';
import Glyph from '$lib/components/atoms/Glyph.svelte';
import Icon from '$lib/components/atoms/Icon.svelte';
import IconButton from '$lib/components/atoms/IconButton.svelte';
import KeyHint from '$lib/components/atoms/KeyHint.svelte';
import StatusDot from '$lib/components/atoms/StatusDot.svelte';
import CommandSearch from '$lib/components/molecules/CommandSearch.svelte';
import FloatCard from '$lib/components/molecules/FloatCard.svelte';
import PanelRow from '$lib/components/molecules/PanelRow.svelte';
import PanelSection from '$lib/components/molecules/PanelSection.svelte';
import BoardCanvas from '$lib/components/organisms/BoardCanvas.svelte';
import CamObjectBrowser from '$lib/components/organisms/CamObjectBrowser.svelte';
import OperationInspector from '$lib/components/organisms/OperationInspector.svelte';
import SettingsDialog from '$lib/components/organisms/SettingsDialog.svelte';
import StartScreen from '$lib/components/organisms/StartScreen.svelte';
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
    purpose: 'Labelled button with the accent variants and both sizes.',
    path: 'src/lib/components/atoms/Button.svelte',
    component: asCatalogComponent(Button),
    renderable: true,
    states: [
      {
        label: 'primary',
        props: { variant: 'primary' },
        content: 'Generate plan'
      },
      {
        label: 'secondary',
        props: { variant: 'secondary' },
        content: 'Import'
      },
      { label: 'ghost', props: { variant: 'ghost' }, content: 'Cancel' },
      {
        label: 'sm',
        props: { variant: 'secondary', size: 'sm' },
        content: 'Save'
      },
      {
        label: 'md',
        props: { variant: 'secondary', size: 'md' },
        content: 'Save'
      },
      {
        label: 'disabled',
        props: { variant: 'secondary', disabled: true },
        content: 'Layers'
      }
    ]
  },
  {
    slug: 'glyph',
    name: 'Glyph',
    layer: 'atom',
    purpose:
      'Local 24×24 glyph the preferences dialog uses for its close cross.',
    path: 'src/lib/components/atoms/Glyph.svelte',
    component: asCatalogComponent(Glyph),
    renderable: true,
    states: [
      { label: 'md', props: { name: 'close' } },
      { label: 'sm', props: { name: 'close', size: 'sm' } },
      { label: 'lg', props: { name: 'close', size: 'lg' } }
    ]
  },
  {
    slug: 'icon',
    name: 'Icon',
    layer: 'atom',
    purpose:
      '24×24 drawing from the icon set; decorative unless it receives label.',
    path: 'src/lib/components/atoms/Icon.svelte',
    component: asCatalogComponent(Icon),
    renderable: true,
    states: [
      { label: 'sm', props: { name: 'search', size: 'sm' } },
      { label: 'md', props: { name: 'search' } },
      { label: 'lg', props: { name: 'search', size: 'lg' } },
      {
        label: 'with label',
        props: { name: 'settings', label: 'Settings' }
      }
    ]
  },
  {
    slug: 'icon-button',
    name: 'IconButton',
    layer: 'atom',
    purpose: 'Square icon-only control; the accessible name is required.',
    path: 'src/lib/components/atoms/IconButton.svelte',
    component: asCatalogComponent(IconButton),
    renderable: true,
    states: [
      { label: 'md', props: { ariaLabel: 'Open', icon: 'folder' } },
      {
        label: 'sm',
        props: { ariaLabel: 'Save', icon: 'save', size: 'sm' }
      },
      {
        label: 'disabled',
        props: { ariaLabel: 'Layers', icon: 'layers', disabled: true }
      }
    ]
  },
  {
    slug: 'key-hint',
    name: 'KeyHint',
    layer: 'atom',
    purpose: 'Keyboard shortcut chip of the app chrome.',
    path: 'src/lib/components/atoms/KeyHint.svelte',
    component: asCatalogComponent(KeyHint),
    renderable: true,
    states: [
      { label: 'default', props: {} },
      { label: 'custom', props: { keys: 'Esc' } }
    ]
  },
  {
    slug: 'status-dot',
    name: 'StatusDot',
    layer: 'atom',
    purpose:
      'Tonal dot; without label it is decorative and does not communicate by colour.',
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
      { label: 'with label', props: { tone: 'ok', label: 'Status: valid' } }
    ]
  },
  {
    slug: 'command-search',
    name: 'CommandSearch',
    layer: 'molecule',
    purpose:
      'App bar search field; without onquery it stays disabled and says why.',
    path: 'src/lib/components/molecules/CommandSearch.svelte',
    component: asCatalogComponent(CommandSearch),
    renderable: true,
    states: [
      { label: 'without handler', props: {} },
      {
        label: 'enabled',
        props: { onquery: noopQuery, placeholder: 'Search command…' }
      }
    ]
  },
  {
    slug: 'float-card',
    name: 'FloatCard',
    layer: 'molecule',
    purpose: 'Floating surface with a header and a scrollable body.',
    path: 'src/lib/components/molecules/FloatCard.svelte',
    component: asCatalogComponent(FloatCard),
    renderable: true,
    states: [
      {
        label: 'without meta',
        props: { title: 'Inspector' },
        content: 'Card content.'
      },
      {
        label: 'with meta',
        props: { title: 'Browser', meta: '3 layers' },
        content: 'Card content.'
      }
    ]
  },
  {
    slug: 'panel-row',
    name: 'PanelRow',
    layer: 'molecule',
    purpose: 'Panel row with a dot or icon, a label and meta.',
    path: 'src/lib/components/molecules/PanelRow.svelte',
    component: asCatalogComponent(PanelRow),
    renderable: true,
    states: [
      { label: 'label', props: { label: 'Project', meta: 'board.cmc' } },
      {
        label: 'with dot',
        props: {
          label: 'Board',
          meta: '2 layers',
          dot: 'ok',
          dotLabel: 'Status: valid'
        }
      },
      {
        label: 'with icon',
        props: { label: 'Operations', meta: '1', icon: 'layers' }
      },
      {
        label: 'selected',
        props: {
          label: 'Size',
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
      'Panel block with a header, a count and a body, optionally collapsible.',
    path: 'src/lib/components/molecules/PanelSection.svelte',
    component: asCatalogComponent(PanelSection),
    renderable: true,
    states: [
      {
        label: 'with count',
        props: { title: 'Board', count: 2 },
        content: 'Section rows.'
      },
      {
        label: 'collapsible',
        props: {
          title: 'Setup',
          collapsible: true,
          collapsed: false,
          ontoggle: noop
        },
        content: 'Section rows.'
      },
      {
        label: 'collapsed',
        props: {
          title: 'Output',
          collapsible: true,
          collapsed: true,
          ontoggle: noop
        },
        content: 'Section rows.'
      }
    ]
  },
  {
    slug: 'board-canvas',
    name: 'BoardCanvas',
    layer: 'organism',
    purpose: 'three.js viewport of the board with the real scene snapshot.',
    path: 'src/lib/components/organisms/BoardCanvas.svelte',
    component: asCatalogComponent(BoardCanvas),
    renderable: false,
    reason:
      'Needs a WebGL canvas and the live workbench store; does not render in isolation.',
    states: [{ label: 'store and canvas state', props: {} }]
  },
  {
    slug: 'cam-object-browser',
    name: 'CamObjectBrowser',
    layer: 'organism',
    purpose: 'Workbench browser organised by CAM object.',
    path: 'src/lib/components/organisms/CamObjectBrowser.svelte',
    component: asCatalogComponent(CamObjectBrowser),
    renderable: false,
    reason:
      'Reads the live workbench store and the open document; does not render in isolation.',
    states: [
      { label: 'without handler', props: {} },
      { label: 'with onOpenRecent', props: { onOpenRecent: noop } }
    ]
  },
  {
    slug: 'operation-inspector',
    name: 'OperationInspector',
    layer: 'organism',
    purpose: 'Inspector of the selected operation and the workbench state.',
    path: 'src/lib/components/organisms/OperationInspector.svelte',
    component: asCatalogComponent(OperationInspector),
    renderable: false,
    reason: 'Reads the live workbench store; does not render in isolation.',
    states: [{ label: 'store state', props: {} }]
  },
  {
    slug: 'settings-dialog',
    name: 'SettingsDialog',
    layer: 'organism',
    purpose:
      'Preferences dialog: reads and writes the backend-persisted theme.',
    path: 'src/lib/components/organisms/SettingsDialog.svelte',
    component: asCatalogComponent(SettingsDialog),
    renderable: false,
    reason:
      'Writes settings/theme over Tauri IPC and reads the live store; does not render in isolation.',
    states: [{ label: 'store state (settingsOpen)', props: {} }]
  },
  {
    slug: 'start-screen',
    name: 'StartScreen',
    layer: 'organism',
    purpose: 'Start screen: file actions, new-document form and recents.',
    path: 'src/lib/components/organisms/StartScreen.svelte',
    component: asCatalogComponent(StartScreen),
    renderable: true,
    states: [
      { label: 'without recents', props: {} },
      {
        label: 'with recents',
        props: {
          recents: [
            {
              name: 'controller-board.cmc',
              path: '/tmp/controller-board.cmc',
              lastOpened: '2026-11-26T00:00:00Z'
            }
          ]
        }
      },
      { label: 'with error', props: { error: 'state poisoned' } },
      { label: 'new project', props: { showNew: true } },
      {
        label: 'with handlers',
        props: {
          onOpen: noop,
          onOpenRecent: noop,
          onCreate: noop,
          onSettings: noop
        }
      }
    ]
  },
  {
    slug: 'status-bar',
    name: 'StatusBar',
    layer: 'organism',
    purpose: 'Status bar with coordinates, selection and workbench health.',
    path: 'src/lib/components/organisms/StatusBar.svelte',
    component: asCatalogComponent(StatusBar),
    renderable: false,
    reason:
      'Reads the live pointer, selection and workbench state; does not render in isolation.',
    states: [{ label: 'store state', props: {} }]
  },
  {
    slug: 'tool-rail',
    name: 'ToolRail',
    layer: 'organism',
    purpose: 'Viewport quick-action rail.',
    path: 'src/lib/components/organisms/ToolRail.svelte',
    component: asCatalogComponent(ToolRail),
    renderable: true,
    states: [
      { label: 'without handler', props: {} },
      { label: 'with onOpen', props: { onOpen: noop } }
    ]
  },
  {
    slug: 'top-bar',
    name: 'TopBar',
    layer: 'organism',
    purpose: 'App bar with brand, project, search and file actions.',
    path: 'src/lib/components/organisms/TopBar.svelte',
    component: asCatalogComponent(TopBar),
    renderable: false,
    reason:
      'The theme action calls update_settings over Tauri IPC and reads the live store; does not render in isolation.',
    states: [{ label: 'without handlers', props: {} }]
  },
  {
    slug: 'viewport-hud',
    name: 'ViewportHud',
    layer: 'organism',
    purpose: 'Viewport HUD cards with the real selection.',
    path: 'src/lib/components/organisms/ViewportHud.svelte',
    component: asCatalogComponent(ViewportHud),
    renderable: false,
    reason: 'Reads the live workbench selection; does not render in isolation.',
    states: [{ label: 'store state', props: {} }]
  },
  {
    slug: 'workbench-template',
    name: 'WorkbenchTemplate',
    layer: 'template',
    purpose: 'App frame with one region per artboard slot.',
    path: 'src/lib/components/templates/WorkbenchTemplate.svelte',
    component: asCatalogComponent(WorkbenchTemplate),
    renderable: true,
    states: [
      { label: 'empty regions', props: {} },
      { label: 'with ariaLabel', props: { ariaLabel: 'Camcraft' } }
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
