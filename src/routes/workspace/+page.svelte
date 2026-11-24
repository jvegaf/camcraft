<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
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
// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
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

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
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

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
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

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function backToStart() {
  await goto('/');
}

void refreshStatus();
</script>

<main class="workspace">
  <header class="topbar">
    <div>
      <p class="eyebrow">Camcraft</p>
      <h1>Workspace</h1>
    </div>
    <div class="actions">
      <button type="button" onclick={backToStart}>Start screen</button>
      <button type="button" onclick={saveDocument}>Save</button>
      <button type="button" class="primary" onclick={saveDocumentAs}>Save as</button>
    </div>
  </header>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if status}
    <section class="panel">
      <h2>Document</h2>
      <dl class="status">
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
    <p class="muted">Loading document…</p>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    background: #eef1f4;
    color: #1f2430;
    font-family: Inter, system-ui, sans-serif;
  }
  .workspace { max-width: 1100px; margin: 0 auto; padding: 32px 20px; }
  .topbar { display: flex; justify-content: space-between; align-items: end; gap: 16px; margin-bottom: 24px; }
  .eyebrow { margin: 0; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; color: #667085; }
  h1 { margin: 8px 0 0; font-size: 36px; line-height: 1; }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; }
  button { font: inherit; font-size: 14px; padding: 9px 14px; border-radius: 8px; border: 1px solid #cfd6df; background: #fff; color: #1f2430; cursor: pointer; }
  button.primary { background: #1f2430; color: #fff; border-color: #1f2430; }
  .panel { background: #fff; border: 1px solid #d7dde5; border-radius: 12px; padding: 20px; }
  h2 { margin: 0 0 16px; font-size: 18px; }
  .status { display: grid; grid-template-columns: 140px 1fr; gap: 8px 16px; margin: 0; }
  dt { color: #5f6b7a; }
  dd { margin: 0; word-break: break-word; }
  .error { color: #b42318; }
  .muted { color: #667085; }
</style>
