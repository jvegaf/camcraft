<script lang="ts">
import { css } from 'styled-system/css';
import { saveThemePreference, toggleTheme } from '$lib/ds/theme';
import { Icon, IconButton } from '../atoms';
import { CommandSearch } from '../molecules';
import { setStatus, workbenchState } from '../workbench/workbench.svelte';

// Organism (095 T3, S4): the app bar of
// `design/camcraft-workbench-v2/02-floating-studio.html` `.topbar` — product mark, current project,
// the command search in the centre and quiet icon actions on the right. It owns no document state:
// the file actions are still the page's handlers (`WorkbenchShell` / `+page.svelte`), and the theme
// action applies *and stores* the theme through the shared mechanism in `src/lib/ds/theme.ts` — the
// `update_settings` path `SettingsDialog` writes with — so the two surfaces can never disagree.
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
  borderRadius: 'panel',
  display: 'grid',
  placeItems: 'center',
  background: 'raised',
  border: '1px solid',
  borderColor: 'border',
  color: 'sel',
  textStyle: 'label',
  fontWeight: 'bold',
  flex: 'none',
  cursor: 'pointer'
});
const product = css({
  textStyle: 'title',
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
  borderRadius: 'panel',
  color: 'text',
  textStyle: 'small',
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
  borderRadius: 'panel',
  border: '1px solid',
  borderColor: 'hairline',
  bg: 'raised',
  textStyle: 'small',
  color: 'textDim',
  '& b': { color: 'text', fontWeight: '500' }
});

async function handleTheme() {
  // Apply first so the bar answers the click immediately, then store the preference: the dialog
  // reads it back through `get_settings`, so a theme that is not stored would be reverted there.
  const next = toggleTheme();
  try {
    await saveThemePreference(next);
  } catch (e) {
    setStatus(`Error: no se pudo guardar el tema (${String(e)})`, 'error');
  }
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
    <IconButton ariaLabel="Guardar" icon="save" onclick={onSave} />
    {#if onSaveAs}
      <IconButton ariaLabel="Guardar como" icon="save-as" onclick={onSaveAs} />
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
