<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { css } from 'styled-system/css';
import { badge, button, input } from 'styled-system/recipes';
import { goto } from '$app/navigation';

type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

let error = $state('');
let recents = $state<RecentProject[]>([]);
let widthMm = $state(1219);
let heightMm = $state(2438);
let thicknessMm = $state(18);

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function refreshRecents() {
  try {
    recents = await invoke<RecentProject[]>('list_recents');
  } catch {
    // non-fatal
  }
}

async function createDocument(event: Event) {
  event.preventDefault();
  error = '';
  try {
    await invoke('new_document', {
      request: {
        widthMm: Number(widthMm),
        heightMm: Number(heightMm),
        thicknessMm: Number(thicknessMm)
      }
    });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

async function openDocument() {
  error = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Camcraft document', extensions: ['cmc', 'json'] }]
    });
    if (typeof selected !== 'string') return;
    await invoke('open_document', { path: selected });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

async function openRecent(path: string) {
  error = '';
  try {
    await invoke('open_document', { path });
    await goto('/workspace');
  } catch (e) {
    error = formatError(e);
  }
}

void refreshRecents();

const shell = css({
  minH: '100vh',
  display: 'grid',
  placeItems: 'center',
  p: '32px',
  bg: 'app',
  color: 'text',
  fontFamily: 'sans'
});
const start = css({
  w: 'min(1040px, 100%)',
  minH: '640px',
  display: 'grid',
  gridTemplateColumns: { base: '1fr', md: '360px 1fr' },
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  overflow: 'hidden',
  boxShadow: '0 20px 60px rgba(0,0,0,.28)'
});
const brand = css({
  p: '32px',
  borderRight: { base: 'none', md: '1px solid' },
  borderBottom: { base: '1px solid', md: 'none' },
  borderColor: 'hairline',
  bg: 'panel'
});
const eyebrow = css({
  m: '0',
  fontSize: '11px',
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: 'textOff'
});
const h1 = css({
  m: '8px 0 0',
  fontSize: '40px',
  lineHeight: '1',
  color: 'textHi',
  fontWeight: '600'
});
const lead = css({ mt: '12px', mb: '24px', color: 'textDim', maxW: '28ch' });
const actions = css({
  display: 'flex',
  gap: '8px',
  flexWrap: 'wrap',
  mb: '24px'
});
const btn = button({ variant: 'solid', size: 'md' });
const btnPrimary = button({ variant: 'primary', size: 'md' });
const errorText = css({ color: 'danger', fontSize: '14px', m: '0 0 16px' });
const label = css({
  m: '0 0 8px',
  fontSize: '12px',
  letterSpacing: '0.04em',
  textTransform: 'uppercase',
  color: 'textOff'
});
const newDoc = css({
  display: 'grid',
  gap: '12px',
  pt: '20px',
  borderTop: '1px solid',
  borderColor: 'hairline'
});
const fields = css({ display: 'grid', gap: '10px' });
const fieldLabel = css({
  display: 'grid',
  gap: '4px',
  fontSize: '13px',
  color: 'textDim'
});
const inputCls = input();
const recentsPane = css({ p: '32px', bg: 'canvas' });
const recentsHead = css({
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  mb: '16px'
});
const h2 = css({
  m: '0',
  fontSize: '20px',
  color: 'textHi',
  fontWeight: '600'
});
const count = badge();
const muted = css({ color: 'textDim' });
const recentList = css({
  listStyle: 'none',
  m: '0',
  p: '0',
  display: 'grid',
  gap: '8px'
});
const recentItem = css({
  w: '100%',
  textAlign: 'left',
  p: '12px 14px',
  bg: 'panel',
  border: '1px solid',
  borderColor: 'hairline',
  borderRadius: '10px',
  cursor: 'pointer',
  _hover: { borderColor: 'border', bg: 'raised' }
});
const recentName = css({
  display: 'block',
  fontSize: '14px',
  fontWeight: '600',
  color: 'textHi'
});
const recentPath = css({
  display: 'block',
  mt: '4px',
  fontSize: '12px',
  color: 'textDim',
  wordBreak: 'break-all'
});
</script>

<main class={shell}>
  <section class={start}>
    <aside class={brand}>
      <p class={eyebrow}>Camcraft</p>
      <h1 class={h1}>Start</h1>
      <p class={lead}>Open a recent document or create a new board.</p>

      <div class={actions}>
        <button type="button" class={`${btn} ${btnPrimary}`} onclick={openDocument}>Open document</button>
      </div>

      {#if error}
        <p class={errorText}>{error}</p>
      {/if}

      <form class={newDoc} onsubmit={createDocument}>
        <p class={label}>New document</p>
        <div class={fields}>
          <label class={fieldLabel}>Width (mm)<input class={inputCls} type="number" step="any" bind:value={widthMm} /></label>
          <label class={fieldLabel}>Height (mm)<input class={inputCls} type="number" step="any" bind:value={heightMm} /></label>
          <label class={fieldLabel}>Thickness (mm)<input class={inputCls} type="number" step="any" bind:value={thicknessMm} /></label>
        </div>
        <button type="submit" class={`${btn} ${btnPrimary}`}>Create document</button>
      </form>
    </aside>

    <section class={recentsPane}>
      <div class={recentsHead}>
        <h2 class={h2}>Recent</h2>
        <span class={count}>{recents.length}</span>
      </div>
      {#if recents.length === 0}
        <p class={muted}>No recent documents yet.</p>
      {:else}
        <ul class={recentList}>
          {#each recents as recent (recent.path)}
            <li>
              <button type="button" class={recentItem} onclick={() => openRecent(recent.path)}>
                <span class={recentName}>{recent.name}</span>
                <span class={recentPath}>{recent.path}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </section>
</main>
