<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { css } from 'styled-system/css';
import {
  closeSettings,
  workbenchState
} from '$lib/components/workbench/workbench.svelte';
import { applyTheme } from '$lib/ds/theme';

type Settings = { theme: string };
const DEFAULTS: Settings = { theme: 'dark' };

let settings = $state<Settings>({ ...DEFAULTS });
let error = $state('');
let saved = $state(false);
let loaded = $state(false);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function loadSettings() {
  try {
    settings = await invoke<Settings>('get_settings');
    applyTheme(settings.theme);
    loaded = true;
  } catch (e) {
    error = formatError(e);
  }
}

async function saveSettings() {
  error = '';
  saved = false;
  try {
    settings = await invoke<Settings>('update_settings', { settings });
    applyTheme(settings.theme);
    saved = true;
    closeSettings();
  } catch (e) {
    error = formatError(e);
  }
}

function restoreDefaults() {
  settings = { ...DEFAULTS };
  saved = false;
  error = '';
}

$effect(() => {
  if (workbenchState.settingsOpen) {
    saved = false;
    error = '';
    void loadSettings();
  }
});

const overlay = css({
  position: 'fixed',
  inset: '0',
  zIndex: '100',
  display: 'grid',
  placeItems: 'center',
  bg: 'rgba(8,10,14,.45)'
});
const dialog = css({
  w: '920px',
  maxW: 'calc(100vw - 48px)',
  h: '680px',
  maxH: 'calc(100vh - 48px)',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '10px',
  boxShadow: '0 1px 0 rgba(255,255,255,.03), 0 16px 48px rgba(0,0,0,.5)',
  display: 'grid',
  gridTemplateRows: '52px 1fr 56px',
  overflow: 'hidden',
  color: 'text',
  fontFamily: 'sans'
});
const titlebar = css({
  display: 'flex',
  alignItems: 'center',
  gap: '12px',
  px: '16px',
  borderBottom: '1px solid',
  borderColor: 'hairline'
});
const title = css({
  m: '0',
  fontSize: '15px',
  fontWeight: '600',
  color: 'textHi'
});
const search = css({
  ml: 'auto',
  w: '260px',
  h: '30px',
  borderRadius: '6px',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  display: 'flex',
  alignItems: 'center',
  gap: '8px',
  px: '10px',
  color: 'textDim',
  fontSize: '12px',
  '& kbd': {
    ml: 'auto',
    font: '11px ui-monospace, SFMono-Regular, Menlo, monospace',
    color: 'textOff',
    px: '5px',
    py: '1px',
    borderRadius: '3px',
    bg: 'raised',
    border: '1px solid',
    borderColor: 'hairline'
  }
});
const iconBtn = css({
  w: '30px',
  h: '30px',
  display: 'grid',
  placeItems: 'center',
  borderRadius: '6px',
  color: 'textDim',
  cursor: 'pointer',
  bg: 'transparent',
  border: 'none',
  _hover: { bg: 'raised', color: 'textHi' }
});
const content = css({
  display: 'grid',
  gridTemplateColumns: '220px 1fr',
  minH: '0'
});
const pages = css({
  bg: 'rgba(13,16,20,.72)',
  borderRight: '1px solid',
  borderColor: 'hairline',
  p: '12px 8px',
  overflow: 'auto'
});
const group = css({
  m: '10px 8px 6px',
  fontSize: '11px',
  fontWeight: '600',
  letterSpacing: '0.06em',
  textTransform: 'uppercase',
  color: 'textOff'
});
const pageItem = css({
  w: '100%',
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '10px',
  py: '8px',
  borderRadius: '6px',
  textAlign: 'left',
  fontSize: '13px',
  color: 'text',
  bg: 'transparent',
  border: 'none',
  cursor: 'default'
});
const pageItemActive = css({
  bg: 'selSoft',
  color: 'textHi',
  boxShadow: 'inset 0 0 0 1px token(colors.selSoft)'
});
const ic = css({
  w: '18px',
  h: '18px',
  color: 'textDim',
  flex: 'none',
  display: 'grid',
  placeItems: 'center'
});
const icActive = css({ color: 'sel' });
const main = css({ p: '24px 28px', overflow: 'auto', minH: '0' });
const h3 = css({
  m: '0',
  fontSize: '20px',
  fontWeight: '600',
  color: 'textHi'
});
const desc = css({
  m: '6px 0 20px',
  color: 'textDim',
  fontSize: '13px',
  lineHeight: '1.5',
  maxW: '640px'
});
const section = css({
  py: '18px',
  borderTop: '1px solid',
  borderColor: 'hairline'
});
const sectionFirst = css({ borderTop: '0', pt: '0' });
const sectionTitle = css({
  m: '0 0 12px',
  fontSize: '11px',
  fontWeight: '600',
  letterSpacing: '0.06em',
  textTransform: 'uppercase',
  color: 'textOff'
});
const setting = css({
  display: 'grid',
  gridTemplateColumns: '240px 1fr',
  gap: '16px',
  alignItems: 'start',
  py: '10px'
});
const label = css({ fontSize: '13px', color: 'textHi', fontWeight: '500' });
const hint = css({
  fontSize: '12px',
  color: 'textDim',
  mt: '3px',
  lineHeight: '1.45'
});
const selectCls = css({
  w: '100%',
  maxW: '320px',
  h: '32px',
  px: '10px',
  pr: '30px',
  borderRadius: '4px',
  bg: 'app',
  border: '1px solid',
  borderColor: 'border',
  color: 'text',
  fontSize: '13px',
  appearance: 'none',
  backgroundImage:
    'linear-gradient(45deg, transparent 50%, #8b95a3 50%), linear-gradient(135deg, #8b95a3 50%, transparent 50%)',
  backgroundPosition: 'calc(100% - 14px) 13px, calc(100% - 9px) 13px',
  backgroundSize: '5px 5px, 5px 5px',
  backgroundRepeat: 'no-repeat'
});
const footer = css({
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  px: '16px',
  borderTop: '1px solid',
  borderColor: 'hairline',
  bg: 'rgba(23,27,33,.5)'
});
const footerSpacer = css({ flex: '1' });
const btn = css({
  h: '32px',
  px: '14px',
  borderRadius: '6px',
  fontSize: '13px',
  fontWeight: '500',
  border: '1px solid',
  borderColor: 'border',
  bg: 'raised',
  color: 'text',
  cursor: 'pointer',
  _hover: { bg: 'overlay' }
});
const btnPrimary = css({ bg: 'cutSoft', color: 'cut', borderColor: 'cut' });
const btnGhost = css({
  bg: 'transparent',
  borderColor: 'transparent',
  color: 'textDim',
  _hover: { bg: 'raised', color: 'text' }
});
const msgErr = css({ color: 'danger', fontSize: '12px', m: '0' });
const msgOk = css({ color: 'ok', fontSize: '12px', m: '0' });
const icon = css({
  w: '16px',
  h: '16px',
  stroke: 'currentColor',
  fill: 'none',
  strokeWidth: '1.6',
  strokeLinecap: 'round',
  strokeLinejoin: 'round'
});
</script>

{#if workbenchState.settingsOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class={overlay} onclick={(e) => { if (e.target === e.currentTarget) closeSettings(); }}>
    <div class={dialog} role="dialog" aria-label="Preferencias">
      <div class={titlebar}>
        <h2 class={title}>Preferencias</h2>
        <div class={search} role="search">
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>
          Buscar configuración…
          <kbd>⌘F</kbd>
        </div>
        <button class={iconBtn} aria-label="Cerrar" onclick={closeSettings}>
          <svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>
        </button>
      </div>

      <div class={content}>
        <nav class={pages} aria-label="Páginas de preferencias">
          <div class={group}>General</div>
          <button class={`${pageItem} ${pageItemActive}`} aria-current="page">
            <span class={`${ic} ${icActive}`}><svg class={icon} viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v14H4z"/><path d="M8 9h8M8 13h5"/></svg></span>
            Apariencia
          </button>
        </nav>

        <section class={main}>
          <h3 class={h3}>Apariencia</h3>
          <p class={desc}>Configuración visual de Camcraft. Se muestran solo las opciones soportadas actualmente por el backend.</p>

          <div class={`${section} ${sectionFirst}`}>
            <h4 class={sectionTitle}>Tema</h4>
            <div class={setting}>
              <div>
                <div class={label}>Theme</div>
                <div class={hint}>Elegí la apariencia por defecto de Camcraft.</div>
              </div>
              <div>
                <select class={selectCls} bind:value={settings.theme} aria-label="Theme">
                  <option value="dark">Dark</option>
                  <option value="light">Light</option>
                  <option value="system">System</option>
                </select>
              </div>
            </div>
          </div>

          {#if error}<p class={msgErr}>{error}</p>{/if}
          {#if saved}<p class={msgOk}>Settings guardadas.</p>{/if}
        </section>
      </div>

      <div class={footer}>
        <button class={`${btn} ${btnGhost}`} onclick={restoreDefaults}>Restaurar valores</button>
        <div class={footerSpacer}></div>
        <button class={btn} onclick={closeSettings}>Cancelar</button>
        <button class={`${btn} ${btnPrimary}`} onclick={saveSettings}>Aplicar</button>
      </div>
    </div>
  </div>
{/if}
