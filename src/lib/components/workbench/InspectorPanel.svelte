<script lang="ts">
import { css } from 'styled-system/css';
import FloatCard from './FloatCard.svelte';

interface Field {
  label: string;
  value: string;
  unit?: string;
}

interface Props {
  title?: string;
  fields?: Field[];
  stats?: { label: string; value: string }[];
  statusTitle?: string;
  statusText?: string;
}

let {
  title = 'Drill 0.8 / 1.0',
  fields = [
    { label: 'Tipo', value: 'Drill cycle', unit: 'G81' },
    { label: 'Herramienta', value: 'T1 Flat 0.8 mm', unit: 'cambiar' },
    { label: 'Z cut', value: '-1.8', unit: 'mm' },
    { label: 'Feed plunge', value: '120', unit: 'mm/min' }
  ],
  stats = [
    { label: 'Hits', value: '6' },
    { label: 'Herramientas', value: 'T1, T2' },
    { label: 'Tiempo', value: '0:42' }
  ],
  statusTitle = 'Listo para simular.',
  statusText = 'Toolpath válido, sin colisiones detectadas en preview.'
}: Props = $props();

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
</script>

<FloatCard title="Inspector" meta={title} class={panelPos} ariaLabel="Inspector">
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Operación</h3>
    {#each fields as field}
      <div class={fieldCls}>
        <span class={labelCls}>{field.label}</span>
        <div class={inputCls}>{field.value}{#if field.unit}<span class={unitCls}>{field.unit}</span>{/if}</div>
      </div>
    {/each}
    <dl class={kv}>
      {#each stats as stat}
        <dt>{stat.label}</dt><dd>{stat.value}</dd>
      {/each}
    </dl>
  </div>
  <div class={sectionCls}>
    <h3 class={sectionTitle}>Estado</h3>
    <div class={stateCard}><strong>{statusTitle}</strong> {statusText}</div>
  </div>
</FloatCard>
