<script lang="ts">
import { css } from 'styled-system/css';

interface RecentItem {
  name: string;
  path: string;
  when: string;
}

interface Props {
  recents?: RecentItem[];
}

let {
  recents = [
    {
      name: 'controller-board.cam',
      path: '~/Boards/controller/',
      when: 'Hace 12 min'
    },
    { name: 'usb-adapter.cam', path: '~/Boards/adapters/', when: 'Ayer' },
    { name: 'front-panel.cam', path: '~/Boards/panel/', when: 'Hace 3 días' }
  ]
}: Props = $props();

const startShell = css({
  h: '100%',
  minH: '520px',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  overflow: 'hidden',
  bg: 'canvas',
  position: 'relative',
  boxShadow: '0 24px 80px rgba(0,0,0,.35)',
  display: 'grid',
  placeItems: 'center',
  color: 'text',
  fontSize: '13px',
  lineHeight: '1.4',
  fontFamily: 'sans',
  _before: {
    content: '""',
    position: 'absolute',
    inset: '0',
    background:
      'linear-gradient(90deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px, linear-gradient(180deg, rgba(38,45,54,.22) 1px, transparent 1px) 0 0/80px 80px'
  }
});
const startCard = css({
  position: 'relative',
  w: '940px',
  maxW: 'calc(100% - 48px)',
  display: 'grid',
  gridTemplateColumns: '1fr 1fr',
  gap: '16px'
});
const panel = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  boxShadow: '0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42)',
  p: '24px'
});
const eyebrow = css({
  fontSize: '11px',
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: 'textOff',
  m: '0 0 6px'
});
const welcome = css({
  m: '0 0 6px',
  fontSize: '28px',
  fontWeight: '600',
  lineHeight: '1.15',
  color: 'textHi'
});
const heading = css({
  m: '0',
  fontSize: '20px',
  fontWeight: '600',
  color: 'textHi'
});
const lead = css({
  mt: '8px',
  mb: '0',
  color: 'textDim',
  fontSize: '13px',
  lineHeight: '1.5'
});
const action = css({
  w: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  p: '14px',
  mt: '12px',
  borderRadius: '8px',
  bg: 'raised',
  border: '1px solid',
  borderColor: 'hairline',
  textAlign: 'left',
  cursor: 'pointer',
  color: 'inherit',
  _hover: { borderColor: 'sel', bg: 'overlay' }
});
const actionIcon = css({
  w: '34px',
  h: '34px',
  borderRadius: '8px',
  display: 'grid',
  placeItems: 'center',
  bg: 'selSoft',
  color: 'sel',
  flex: 'none'
});
const actionIconPrimary = css({ bg: 'cutSoft', color: 'cut' });
const actionTitle = css({
  display: 'block',
  fontSize: '14px',
  color: 'textHi',
  fontWeight: '500'
});
const actionDesc = css({
  display: 'block',
  fontSize: '12px',
  color: 'textDim',
  mt: '2px'
});
const recentBtn = css({
  w: '100%',
  display: 'flex',
  gap: '12px',
  alignItems: 'center',
  py: '10px',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  textAlign: 'left',
  bg: 'transparent',
  borderX: 'none',
  borderTop: 'none',
  cursor: 'pointer',
  color: 'inherit',
  _last: { borderBottom: '0', pb: '0' },
  '&:hover .recent-name': { color: 'textHi' }
});
const recentName = css({ fontSize: '13px', color: 'text', fontWeight: '500' });
const recentPath = css({
  fontSize: '11px',
  color: 'textOff',
  fontFamily: 'mono',
  mt: '2px'
});
const recentWhen = css({ ml: 'auto', fontSize: '11px', color: 'textOff' });
const icon = css({
  w: '16px',
  h: '16px',
  stroke: 'currentColor',
  fill: 'none',
  strokeWidth: '1.6',
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  flex: 'none'
});
</script>

<div class={startShell}>
  <div class={startCard}>
    <div class={panel}>
      <p class={eyebrow}>Camcraft</p>
      <h2 class={welcome}>Bienvenido al estudio CAM</h2>
      <p class={lead}>Abrí una placa o retomá un proyecto. El shell usa superficies flotantes, jerarquía de bordes clara y command search central.</p>
      <button class={action}>
        <span class={`${actionIcon} ${actionIconPrimary}`}>
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
        </span>
        <div><strong class={actionTitle}>Nuevo proyecto PCB</strong><span class={actionDesc}>Configurar stock, origen y board</span></div>
      </button>
      <button class={action}>
        <span class={actionIcon}>
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h6l2 2h8v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/></svg>
        </span>
        <div><strong class={actionTitle}>Abrir proyecto</strong><span class={actionDesc}>.cam o fuentes de placa</span></div>
      </button>
    </div>
    <div class={panel}>
      <h2 class={heading}>Recientes</h2>
      <p class={lead}>Retomá con contexto visible antes de abrir.</p>
      {#each recents as recent}
        <button class={recentBtn}>
          <div><div class={`${recentName} recent-name`}>{recent.name}</div><div class={recentPath}>{recent.path}</div></div>
          <div class={recentWhen}>{recent.when}</div>
        </button>
      {/each}
    </div>
  </div>
</div>
