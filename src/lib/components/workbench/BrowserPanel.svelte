<script lang="ts">
import { css } from 'styled-system/css';
import FloatCard from './FloatCard.svelte';

interface NavItem {
  label: string;
  meta?: string;
  dot?: 'copper' | 'drill' | 'cut' | 'ok' | 'default';
  active?: boolean;
}

interface Section {
  title: string;
  items: NavItem[];
}

interface Props {
  sections?: Section[];
}

let {
  sections = [
    {
      title: 'Board',
      items: [
        { label: 'Top Copper', meta: '.gbr', dot: 'copper' },
        { label: 'Bottom Copper', meta: '.gbr', dot: 'copper' },
        { label: 'Drill Hits', meta: '.drl', dot: 'drill', active: true },
        { label: 'Board Outline', meta: '.gbr', dot: 'default' }
      ]
    },
    {
      title: 'Setup',
      items: [
        { label: 'Stock FR-4 1.6mm', meta: '100×80', dot: 'ok' },
        { label: 'Origen bottom-left', meta: 'X0 Y0', dot: 'ok' }
      ]
    },
    {
      title: 'Operaciones',
      items: [
        { label: '1 · Profile outline', meta: 'OK', dot: 'cut' },
        {
          label: '2 · Drill 0.8 / 1.0',
          meta: 'seleccionada',
          dot: 'drill',
          active: true
        },
        { label: '3 · Isolation top', meta: 'draft', dot: 'copper' }
      ]
    },
    {
      title: 'Salida',
      items: [{ label: 'Post GRBL', meta: '.nc', dot: 'default' }]
    }
  ]
}: Props = $props();

const panelPos = css({
  top: '64px',
  left: '16px',
  w: '284px',
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
const navItem = css({
  w: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  px: '8px',
  py: '6px',
  borderRadius: '5px',
  textAlign: 'left',
  fontSize: '12px',
  color: 'text',
  cursor: 'pointer',
  bg: 'transparent',
  border: 'none',
  _hover: { bg: 'raised' },
  _focusVisible: {
    outline: '2px solid',
    outlineColor: 'sel',
    outlineOffset: '-1px'
  }
});
const navActive = css({
  bg: 'selSoft',
  color: 'textHi',
  boxShadow: 'inset 0 0 0 1px token(colors.selSoft)'
});
const dotBase = css({
  w: '8px',
  h: '8px',
  borderRadius: '50%',
  bg: 'border',
  flex: 'none'
});
const count = css({
  ml: 'auto',
  fontSize: '11px',
  fontFamily: 'mono',
  color: 'textOff',
  whiteSpace: 'nowrap'
});
const dotColor: Record<string, string> = {
  copper: '#c47a2c',
  drill: '#7cc7ff',
  cut: '#f5a623',
  ok: '#5ec27f',
  default: '#35404c'
};
</script>

<FloatCard title="Browser" meta="Proyecto" class={panelPos} ariaLabel="Browser">
  {#each sections as section}
    <div class={sectionCls}>
      <h3 class={sectionTitle}>{section.title}</h3>
      {#each section.items as item}
        <button class={item.active ? `${navItem} ${navActive}` : navItem} aria-current={item.active ? 'true' : undefined}>
          <span class={dotBase} style={`background:${dotColor[item.dot ?? 'default']}`}></span>
          {item.label}
          {#if item.meta}<span class={count}>{item.meta}</span>{/if}
        </button>
      {/each}
    </div>
  {/each}
</FloatCard>
