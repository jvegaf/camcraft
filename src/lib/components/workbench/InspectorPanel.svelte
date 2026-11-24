<script lang="ts">
import { css } from 'styled-system/css';
import FloatCard from './FloatCard.svelte';
import { workbenchState } from './workbench.svelte';

const panelPos = css({
  top: '64px',
  right: '16px',
  w: '324px',
  maxH: 'calc(100% - 114px)'
});
const sectionCls = css({ mb: '12px', _last: { mb: '0' } });
const sectionTitle = css({
  m: '0 0 6px',
  px: '4px',
  fontSize: '11px',
  color: 'textOff',
  fontWeight: '500',
  textTransform: 'uppercase',
  letterSpacing: '0.04em',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  _after: { content: '""', flex: '1', h: '1px', bg: 'hairline', opacity: '0.9' }
});
const fieldCls = css({ display: 'grid', gap: '4px', mb: '10px' });
const labelCls = css({ fontSize: '11px', color: 'textDim', px: '1px' });
const inputCls = css({
  h: '30px',
  px: '10px',
  borderRadius: 'control',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  fontSize: '12px',
  color: 'text'
});
const unitCls = css({ color: 'textOff', fontSize: '11px', fontFamily: 'mono' });
const kv = css({
  display: 'grid',
  gridTemplateColumns: '1fr auto',
  gap: '6px 12px',
  fontSize: '12px',
  pt: '10px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  mt: '2px',
  mb: '0',
  '& dt': { color: 'textDim' },
  '& dd': { m: '0', fontSize: '11px', fontFamily: 'mono', color: 'text' }
});
const stateCard = css({
  p: '10px',
  borderRadius: '6px',
  bg: 'ok/8',
  border: '1px solid',
  borderColor: 'ok/22',
  color: 'text',
  fontSize: '12px',
  lineHeight: '1.45',
  '& strong': { color: 'ok', fontWeight: '500' }
});
const neutralCard = css({
  p: '10px',
  borderRadius: '6px',
  bg: 'raised',
  border: '1px solid',
  borderColor: 'hairline',
  color: 'textDim',
  fontSize: '12px',
  lineHeight: '1.45'
});
</script>

<FloatCard
  title="Inspector"
  meta={workbenchState.selection ? `Tri ${workbenchState.selection.index}` : 'Sin selección'}
  class={panelPos}
  ariaLabel="Inspector"
>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Selección</h3>
    {#if workbenchState.selection}
      <div class={fieldCls}>
        <span class={labelCls}>Tipo</span>
        <div class={inputCls}>Triángulo superficie<span class={unitCls}>mesh</span></div>
      </div>
      <div class={fieldCls}>
        <span class={labelCls}>Índice</span>
        <div class={inputCls}>{workbenchState.selection.index}<span class={unitCls}>tri</span></div>
      </div>
      <div class={fieldCls}>
        <span class={labelCls}>Profundidad media</span>
        <div class={inputCls}>{workbenchState.selection.avgDepthMm.toFixed(2)}<span class={unitCls}>mm</span></div>
      </div>
      <dl class={kv}>
        <dt>Estado</dt><dd>Seleccionado</dd>
        <dt>Fuente</dt><dd>Viewport 3D</dd>
      </dl>
    {:else}
      <div class={neutralCard}>Hacé click en la superficie para seleccionar un triángulo.</div>
    {/if}
  </div>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Estado</h3>
    {#if workbenchState.selection}
      <div class={stateCard}><strong>Selección activa.</strong> Reflejada en viewport y HUD.</div>
    {:else}
      <div class={neutralCard}>Sin selección activa.</div>
    {/if}
  </div>
</FloatCard>
