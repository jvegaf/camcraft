<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { css } from 'styled-system/css';
import { goto } from '$app/navigation';

type DocumentStatus = {
  schema: string;
  documentSchema: string;
  dirty: boolean;
  path: string | null;
  layerCount: number;
  toolpathCount: number;
  boardWidthMm: number;
  boardHeightMm: number;
  boardThicknessMm: number;
};

let status = $state<DocumentStatus | null>(null);
let error = $state('');

function formatError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e && typeof e === 'object' && 'message' in e) {
    const err = e as { code?: string; message: string };
    return err.code ? `${err.code}: ${err.message}` : err.message;
  }
  return String(e);
}

async function refreshStatus() {
  error = '';
  try {
    status = await invoke<DocumentStatus>('document_status');
  } catch (e) {
    error = formatError(e);
  }
}

async function saveDocument() {
  error = '';
  try {
    status = await invoke<DocumentStatus>('save_document', {
      path: status?.path ?? null
    });
  } catch (e) {
    error = formatError(e);
  }
}

async function saveDocumentAs() {
  error = '';
  try {
    const target = await save({
      filters: [{ name: 'Camcraft document', extensions: ['cmc'] }]
    });
    if (!target) return;
    status = await invoke<DocumentStatus>('save_document', { path: target });
  } catch (e) {
    error = formatError(e);
  }
}

async function backToStart() {
  await goto('/');
}

void refreshStatus();

const workspace = css({
  maxW: '1100px',
  mx: 'auto',
  p: '32px 20px',
  color: 'text',
  fontFamily: 'sans'
});
const topbar = css({
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'end',
  gap: '16px',
  mb: '24px'
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
  fontSize: '36px',
  lineHeight: '1',
  color: 'textHi',
  fontWeight: '600'
});
const actions = css({ display: 'flex', gap: '8px', flexWrap: 'wrap' });
const btn = css({
  font: 'inherit',
  fontSize: '14px',
  px: '14px',
  py: '9px',
  borderRadius: '8px',
  border: '1px solid',
  borderColor: 'border',
  bg: 'raised',
  color: 'text',
  cursor: 'pointer',
  _hover: { bg: 'overlay', color: 'textHi' }
});
const btnPrimary = css({ bg: 'selSoft', borderColor: 'sel', color: 'textHi' });
const panel = css({
  bg: 'panel',
  border: '1px solid',
  borderColor: 'border',
  borderRadius: '12px',
  p: '20px'
});
const h2 = css({
  m: '0 0 16px',
  fontSize: '18px',
  color: 'textHi',
  fontWeight: '600'
});
const statusGrid = css({
  display: 'grid',
  gridTemplateColumns: '140px 1fr',
  gap: '8px 16px',
  m: '0',
  '& dt': { color: 'textDim' },
  '& dd': { m: '0', wordBreak: 'break-word', color: 'text' }
});
const errorText = css({ color: 'danger' });
const muted = css({ color: 'textDim' });
</script>

<main class={workspace}>
  <header class={topbar}>
    <div>
      <p class={eyebrow}>Camcraft</p>
      <h1 class={h1}>Workspace</h1>
    </div>
    <div class={actions}>
      <button type="button" class={btn} onclick={backToStart}>Start screen</button>
      <button type="button" class={btn} onclick={saveDocument}>Save</button>
      <button type="button" class={`${btn} ${btnPrimary}`} onclick={saveDocumentAs}>Save as</button>
    </div>
  </header>

  {#if error}
    <p class={errorText}>{error}</p>
  {/if}

  {#if status}
    <section class={panel}>
      <h2 class={h2}>Document</h2>
      <dl class={statusGrid}>
        <dt>Schema</dt><dd>{status.documentSchema}</dd>
        <dt>Path</dt><dd>{status.path ?? '(unsaved)'}</dd>
        <dt>Dirty</dt><dd>{status.dirty ? 'yes' : 'no'}</dd>
        <dt>Layers</dt><dd>{status.layerCount}</dd>
        <dt>Toolpaths</dt><dd>{status.toolpathCount}</dd>
        <dt>Board</dt>
        <dd>{status.boardWidthMm} × {status.boardHeightMm} × {status.boardThicknessMm} mm</dd>
      </dl>
    </section>
  {:else}
    <p class={muted}>Loading document…</p>
  {/if}
</main>
