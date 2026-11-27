<script lang="ts">
import { css } from 'styled-system/css';
import type { StatusDotTone } from '../atoms/tone';
import { FloatCard, PanelRow, PanelSection } from '../molecules';
import type { CamOperation } from '../workbench/workbench.svelte';
import {
  isSectionCollapsed,
  selectBoardObject,
  selectOperation,
  toggleSection,
  workbenchState
} from '../workbench/workbench.svelte';

// Organism (095 T4, S5 + S7) — the Browser of
// `design/camcraft-workbench-v2/02-floating-studio.html` (`.float.browser`), organised by CAM object
// instead of by CAD feature: «Browser», the «Proyecto» row, then «Board», «Setup», «Operaciones» and
// «Salida» (PanelSection + PanelRow from 095 T1).
//
// Every row comes from data the app really has. What it does not have is reported, never filled in:
//   · Board — `document_status` gives the board geometry and `layerCount`
//     (`src-tauri/src/lib.rs:145-156`); it does not give layer names, so the count sits in the
//     section header and the artboard's "Top Copper .gbr" rows are not invented.
//   · Setup — no command exposes the work origin or the stock material (`canonical_job()` inside
//     `src-tauri/src/lib.rs:387-396` hardcodes `OriginCorner::BottomLeft` without serialising it).
//   · Operaciones — `toolpathCount` is real, the names and statuses are not exposed by any command,
//     so the list stays empty until the store is fed and the section says so.
//   · Salida — there is no export/post command in `src-tauri/src/lib.rs:496-505`.
//   · Recientes — 095 T5 (L8e) restores the capability T4 dropped: the rows are exactly the
//     `list_recents` entries the shell already loads into `workbenchState.recents`, capped at the
//     same five the pre-T4 panel showed, and the section disappears entirely when there are none so
//     the artboard's four-section IA holds on an empty list. No command is added and no entry is
//     invented; activating a row only forwards its path to the shell's handler.
// Collapsed sections and the selected board object/operation are workbench store state (S7), so this
// organism reads the store and only ever calls its setters: no Tauri invoke lands here.
type Props = {
  /** Reopens a recent project. The shell owns the Tauri call; this organism only forwards the path. */
  onOpenRecent?: (path: string) => void;
};

let { onOpenRecent }: Props = $props();

type BoardObjectRow = { id: string; label: string; value: string };
const MAX_RECENTS = 5;

const panelPos = css({
  top: '64px',
  left: '16px',
  w: '284px',
  maxH: 'calc(100% - 114px)'
});
const empty = css({
  px: '8px',
  py: '6px',
  fontSize: '12px',
  lineHeight: '1.45',
  color: 'textOff'
});

let doc = $derived(workbenchState.document);
let projectName = $derived(doc?.path?.split('/').pop() ?? '—');
// The dot carries the only board-level state the app knows: whether the document has unsaved edits.
let projectDot = $derived<StatusDotTone | undefined>(
  doc ? (doc.dirty ? 'warn' : 'ok') : undefined
);
let boardObjects = $derived<BoardObjectRow[]>(
  doc
    ? [
        {
          id: 'size',
          label: 'Size',
          value: `${doc.boardWidthMm}×${doc.boardHeightMm} mm`
        },
        {
          id: 'thickness',
          label: 'Thickness',
          value: `${doc.boardThicknessMm} mm`
        }
      ]
    : []
);
let operations = $derived<CamOperation[]>(workbenchState.operations);
let recents = $derived(workbenchState.recents.slice(0, MAX_RECENTS));
</script>

<FloatCard title="Browser" class={panelPos} ariaLabel="Browser">
  <PanelRow label="Project" meta={projectName} dot={projectDot} />

  <PanelSection
    title="Board"
    count={doc?.layerCount}
    collapsible
    collapsed={isSectionCollapsed('board')}
    ontoggle={() => toggleSection('board')}
  >
    {#if boardObjects.length === 0}
      <p class={empty} data-empty="board"
        >No document open: open or create a document to see its board.</p
      >
    {:else}
      {#each boardObjects as object (object.id)}
        <PanelRow
          label={object.label}
          meta={object.value}
          selected={workbenchState.selectedBoardObjectId === object.id}
          onclick={() => selectBoardObject(object.id)}
        />
      {/each}
    {/if}
  </PanelSection>

  <PanelSection
    title="Setup"
    collapsible
    collapsed={isSectionCollapsed('setup')}
    ontoggle={() => toggleSection('setup')}
  >
    <p class={empty} data-empty="setup"
      >No setup data: the work origin and the stock material are not exposed by the
      backend.</p
    >
  </PanelSection>

  <PanelSection
    title="Operations"
    count={doc?.toolpathCount}
    collapsible
    collapsed={isSectionCollapsed('operations')}
    ontoggle={() => toggleSection('operations')}
  >
    {#if operations.length > 0}
      {#each operations as operation (operation.id)}
        <PanelRow
          label={`${operation.index} · ${operation.name ?? '—'}`}
          selected={workbenchState.selectedOperationId === operation.id}
          onclick={() => selectOperation(operation.id)}
        />
      {/each}
    {:else if !doc}
      <p class={empty} data-empty="operations"
        >No document open: there are no operations to list.</p
      >
    {:else if doc.toolpathCount === 0}
      <p class={empty} data-empty="operations"
        >No operations: the document has no toolpaths.</p
      >
    {:else}
      <p class={empty} data-empty="operations"
        >{doc.toolpathCount} operations without detail: the document exposes the count,
        not the names or the states.</p
      >
    {/if}
  </PanelSection>

  <PanelSection
    title="Output"
    collapsible
    collapsed={isSectionCollapsed('output')}
    ontoggle={() => toggleSection('output')}
  >
    <p class={empty} data-empty="output"
      >No output generated: the backend exposes neither export nor post-processor.</p
    >
  </PanelSection>

  {#if recents.length > 0}
    <PanelSection title="Recent">
      {#each recents as recent (recent.path)}
        <PanelRow
          label={recent.name}
          onclick={() => onOpenRecent?.(recent.path)}
        />
      {/each}
    </PanelSection>
  {/if}
</FloatCard>
