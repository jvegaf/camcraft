<script lang="ts">
import { css } from 'styled-system/css';
import { toggleTheme } from '$lib/ds/theme';
import { Icon, IconButton } from '../atoms';
import { CommandSearch } from '../molecules';
import { workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T3, S4): the app bar of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.topbar` — product mark, current project,
// the command search in the centre and quiet icon actions on the right. It owns no document state:
// the file actions are still the page's handlers (`WorkbenchShell` / `+page.svelte`), and the theme
// action uses the same shared mechanism as `SettingsDialog`.
interface Props {
  projectName?: string;
  onNew?: () => void;
  onOpen?: () => void;
  onSave?: () => void;
  onSaveAs?: () => void;
  onSettings?: () => void;
  onBack?: () => void;
}

let {
  projectName = 'controller-board.cam',
  onNew,
  onOpen,
  onSave,
  onSaveAs,
  onSettings,
  onBack
}: Props = $props();

const topbar = css({
  position: 'absolute',
  top: '0',
  left: '0',
  right: '0',
  h: '48px',
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '12px',
  background: 'panel',
  backdropFilter: 'blur(8px)',
  borderBottom: '1px solid',
  borderColor: 'hairline',
  zIndex: '30'
});
const mark = css({
  w: '26px',
  h: '26px',
  borderRadius: '7px',
  display: 'grid',
  placeItems: 'center',
  background: 'raised',
  border: '1px solid',
  borderColor: 'border',
  color: 'sel',
  fontSize: '12px',
  fontWeight: '700',
  flex: 'none',
  cursor: 'pointer'
});
const product = css({
  fontSize: '15px',
  fontWeight: '600',
  color: 'textHi',
  letterSpacing: '-0.01em'
});
const sep = css({ w: '1px', h: '18px', bg: 'hairline', flex: 'none' });
const workspace = css({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  px: '8px',
  py: '5px',
  borderRadius: '6px',
  color: 'text',
  fontSize: '12px',
  border: '1px solid transparent',
  cursor: 'pointer',
  _hover: { bg: 'raised', borderColor: 'hairline' }
});
const muted = css({ color: 'textOff' });
// `.search { margin:0 auto }` — the command search is the centred element of the bar.
const search = css({ mx: 'auto' });
const topActions = css({ display: 'flex', alignItems: 'center', gap: '4px' });
const layoutPill = css({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
  px: '8px',
  py: '4px',
  borderRadius: '6px',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'raised',
  fontSize: '11px',
  color: 'textDim',
  '& b': { color: 'text', fontWeight: '500' }
});

function handleTheme() {
  // Same shared mechanism `SettingsDialog` writes with; the bar only flips the applied theme, so it
  // never pretends a preference was stored.
  toggleTheme();
}
</script>

<header class={topbar}>
  <button class={mark} aria-label="Inicio" onclick={onBack}>C</button>
  <div class={product}>Camcraft</div>
  <div class={sep} aria-hidden="true"></div>
  <button class={workspace} aria-label="Proyecto actual" onclick={onOpen}>
    <span>{projectName}{workbenchState.document?.dirty ? ' •' : ''}</span>
    <span class={muted}>▾</span>
  </button>
  <CommandSearch class={search} />
  <div class={topActions}>
    <IconButton ariaLabel="Nuevo" icon="plus" onclick={onNew} />
    <IconButton ariaLabel="Abrir" icon="folder" onclick={onOpen} />
    <IconButton ariaLabel="Guardar" icon="import" onclick={onSave} />
    {#if onSaveAs}
      <IconButton ariaLabel="Guardar como" icon="import" onclick={onSaveAs} />
    {/if}
    <div class={layoutPill} aria-label="Layout flotante"><b>Floating</b> layout</div>
    <IconButton ariaLabel="Tema" icon="theme" onclick={handleTheme} />
    <IconButton
      ariaLabel="Configuración"
      icon="settings"
      onclick={onSettings}
    />
  </div>
</header>
