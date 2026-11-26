<script lang="ts">
import { css, cx } from 'styled-system/css';
import { FloatCard, PanelSection } from '../molecules';
import { workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T4, S5 + S7) — the Inspector of
// `design/camcraft-workbench-v2/02-floating-studio.html` (`.float.inspector`) as an *operation*
// inspector: «Operación» (Tipo, Herramienta, Z cut, Feed plunge, Hits, Herramientas, Tiempo) plus the
// «Estado» block, with the artboard's exact labels.
//
// No command exposes an operation yet: `document_status` only carries `toolpathCount`
// (`src-tauri/src/lib.rs:145-156`) and `scene_snapshot` carries the mesh. So the store's operation
// list is empty in the app today and this panel shows an honest empty state; when an operation is
// selected, every field the app does not know renders an em dash instead of the artboard's sample
// numbers (`-1.8 mm`, `120 mm/min`, `0:42`). The «Estado» block prints the real workbench status.
const UNKNOWN = '—';

const panelPos = css({
  top: '64px',
  right: '16px',
  w: '324px',
  maxH: 'calc(100% - 114px)'
});
const field = css({ display: 'grid', gap: '4px', mb: '10px' });
const fieldLabel = css({
  textStyle: 'small',
  color: 'textDim',
  px: '1px'
});
const fieldValue = css({
  h: '30px',
  px: '10px',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: '8px',
  borderRadius: 'control',
  background: 'app',
  border: '1px solid',
  borderColor: 'border',
  textStyle: 'small',
  color: 'text'
});
const unit = css({
  flex: 'none',
  textStyle: 'mono',
  color: 'textOff'
});
const kv = css({
  display: 'grid',
  gridTemplateColumns: '1fr auto',
  gap: '6px 12px',
  m: '0',
  pt: '10px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  textStyle: 'small',
  '& dt': { color: 'textDim' },
  '& dd': { m: '0', textStyle: 'mono', color: 'text' }
});
const empty = css({
  px: '8px',
  py: '6px',
  textStyle: 'small',
  lineHeight: '1.45',
  color: 'textOff'
});
const stateCard = css({
  p: '10px',
  borderRadius: 'panel',
  border: '1px solid',
  textStyle: 'small',
  lineHeight: '1.45',
  color: 'text'
});
const stateOk = css({ background: 'ok/8', borderColor: 'ok/22' });
const stateError = css({ background: 'danger/8', borderColor: 'danger/22' });
const stateInfo = css({
  background: 'raised',
  borderColor: 'hairline',
  color: 'textDim'
});

let operation = $derived(
  workbenchState.operations.find(
    candidate => candidate.id === workbenchState.selectedOperationId
  ) ?? null
);
let stateClass = $derived(
  cx(
    stateCard,
    workbenchState.statusKind === 'ok'
      ? stateOk
      : workbenchState.statusKind === 'error'
        ? stateError
        : stateInfo
  )
);
let toolsLabel = $derived(
  operation?.tools && operation.tools.length > 0
    ? operation.tools.join(', ')
    : UNKNOWN
);

/** The artboard prints `0:42`; minutes plus zero-padded seconds keeps that shape for real values. */
function timecode(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}
</script>

<FloatCard
  title="Inspector"
  meta={operation?.name ?? undefined}
  class={panelPos}
  ariaLabel="Inspector"
>
  <PanelSection title="Operación">
    {#if operation}
      <div class={field}>
        <span class={fieldLabel}>Tipo</span>
        <div class={fieldValue}><span>{operation.type ?? UNKNOWN}</span></div>
      </div>
      <div class={field}>
        <span class={fieldLabel}>Herramienta</span>
        <div class={fieldValue}><span>{operation.tool ?? UNKNOWN}</span></div>
      </div>
      <div class={field}>
        <span class={fieldLabel}>Z cut</span>
        <div class={fieldValue}>
          <span>{operation.zCutMm ?? UNKNOWN}</span>
          {#if operation.zCutMm !== null}<span class={unit}>mm</span>{/if}
        </div>
      </div>
      <div class={field}>
        <span class={fieldLabel}>Feed plunge</span>
        <div class={fieldValue}>
          <span>{operation.plungeFeedMmPerMin ?? UNKNOWN}</span>
          {#if operation.plungeFeedMmPerMin !== null}<span class={unit}>mm/min</span>{/if}
        </div>
      </div>
      <dl class={kv}>
        <dt>Hits</dt>
        <dd>{operation.hitCount ?? UNKNOWN}</dd>
        <dt>Herramientas</dt>
        <dd>{toolsLabel}</dd>
        <dt>Tiempo</dt>
        <dd>{operation.durationS === null ? UNKNOWN : timecode(operation.durationS)}</dd>
      </dl>
    {:else}
      <p class={empty} data-empty="operation"
        >Sin operación seleccionada. El documento expone el conteo de toolpaths, no sus
        nombres ni sus parámetros.</p
      >
    {/if}
  </PanelSection>

  <PanelSection title="Estado">
    <div class={stateClass}>{workbenchState.statusMessage}</div>
  </PanelSection>
</FloatCard>
