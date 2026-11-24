<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { css } from 'styled-system/css';
import { button } from 'styled-system/recipes';
import { goto } from '$app/navigation';

type Settings = {
  theme: string;
};

let settings = $state<Settings>({ theme: 'dark' });
let error = $state('');
let saved = $state(false);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

function applyTheme(theme: string) {
  const root = document.documentElement;
  if (theme === 'light' || theme === 'dark') {
    root.dataset.theme = theme;
  } else {
    root.removeAttribute('data-theme');
  }
}

async function loadSettings() {
  try {
    settings = await invoke<Settings>('get_settings');
    applyTheme(settings.theme);
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
  } catch (e) {
    error = formatError(e);
  }
}

void loadSettings();

const shell = css({
  minH: '100vh',
  bg: 'canvas',
  color: 'text',
  fontFamily: 'sans',
  p: '32px 20px'
});
const card = css({
  maxW: '720px',
  mx: 'auto',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  p: '24px',
  boxShadow: '0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42)'
});
const topbar = css({
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  gap: '12px',
  mb: '20px'
});
const eyebrow = css({
  m: '0',
  fontSize: '11px',
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: 'textOff'
});
const h1 = css({
  m: '6px 0 0',
  fontSize: '28px',
  lineHeight: '1.15',
  color: 'textHi',
  fontWeight: '600'
});
const section = css({
  py: '16px',
  borderTop: '1px solid',
  borderColor: 'hairline'
});
const label = css({
  display: 'grid',
  gap: '6px',
  fontSize: '13px',
  color: 'textHi',
  fontWeight: '500'
});
const hint = css({ fontSize: '12px', color: 'textDim', fontWeight: '400' });
const selectCls = css({
  maxW: '280px',
  font: 'inherit',
  px: '10px',
  py: '8px',
  borderRadius: '4px',
  border: '1px solid',
  borderColor: 'border',
  bg: 'app',
  color: 'textHi'
});
const actions = css({ display: 'flex', gap: '8px', mt: '20px' });
const btn = button({ variant: 'solid', size: 'md' });
const btnPrimary = button({ variant: 'primary', size: 'md' });
const errorText = css({ color: 'danger', fontSize: '13px', m: '12px 0 0' });
const okText = css({ color: 'ok', fontSize: '13px', m: '12px 0 0' });
</script>

<main class={shell}>
  <section class={card}>
    <div class={topbar}>
      <div>
        <p class={eyebrow}>Camcraft</p>
        <h1 class={h1}>Settings</h1>
      </div>
      <button type="button" class={btn} onclick={() => goto('/')}>Start screen</button>
    </div>

    <div class={section}>
      <label class={label}>
        <span>Theme</span>
        <span class={hint}>Choose the default appearance for Camcraft.</span>
        <select class={selectCls} bind:value={settings.theme}>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
          <option value="system">System</option>
        </select>
      </label>
    </div>

    {#if error}
      <p class={errorText}>{error}</p>
    {/if}
    {#if saved}
      <p class={okText}>Settings saved.</p>
    {/if}

    <div class={actions}>
      <button type="button" class={`${btn} ${btnPrimary}`} onclick={saveSettings}>Save settings</button>
    </div>
  </section>
</main>
