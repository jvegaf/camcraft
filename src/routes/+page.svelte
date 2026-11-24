<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';

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

type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

let status = $state<DocumentStatus | null>(null);
// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
let error = $state('');
// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
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
    // non-fatal: recents should not block document actions
  }
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
async function createDocument(event: Event) {
  event.preventDefault();
  error = '';
  try {
    status = await invoke<DocumentStatus>('new_document', {
      request: {
        widthMm: Number(widthMm),
        heightMm: Number(heightMm),
        thicknessMm: Number(thicknessMm)
      }
    });
  } catch (e) {
    error = formatError(e);
  }
}

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function openDocument() {
  error = '';
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Camcraft document', extensions: ['cmc', 'json'] }]
    });
    if (typeof selected !== 'string') return;
    status = await invoke<DocumentStatus>('open_document', { path: selected });
    await refreshRecents();
  } catch (e) {
    error = formatError(e);
  }
}

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
async function openRecent(path: string) {
  error = '';
  try {
    status = await invoke<DocumentStatus>('open_document', { path });
    await refreshRecents();
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
    await refreshRecents();
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
    await refreshRecents();
  } catch (e) {
    error = formatError(e);
  }
}

void refreshStatus();
void refreshRecents();
</script>

<main class="container">
  <h1>Camcraft</h1>
  <p class="subtitle">Tauri host + domain Rust spike</p>

  <section class="panel">
    <div class="row">
      <h2>Document</h2>
      <div class="actions">
        <button type="button" onclick={openDocument}>Open</button>
        <button type="button" onclick={saveDocument}>Save</button>
        <button type="button" onclick={saveDocumentAs}>Save as</button>
        <button type="button" onclick={refreshStatus}>Refresh</button>
      </div>
    </div>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    {#if status}
      <dl class="status">
        <dt>Schema</dt><dd>{status.documentSchema}</dd>
        <dt>Path</dt><dd>{status.path ?? '(unsaved)'}</dd>
        <dt>Dirty</dt><dd>{status.dirty ? 'yes' : 'no'}</dd>
        <dt>Layers</dt><dd>{status.layerCount}</dd>
        <dt>Toolpaths</dt><dd>{status.toolpathCount}</dd>
        <dt>Board</dt>
        <dd>{status.boardWidthMm} × {status.boardHeightMm} × {status.boardThicknessMm} mm</dd>
      </dl>
    {:else}
      <p>Loading...</p>
    {/if}
  </section>

  <section class="panel">
    <h2>New document</h2>
    <form class="form" onsubmit={createDocument}>
      <label>Width (mm)<input type="number" step="any" bind:value={widthMm} /></label>
      <label>Height (mm)<input type="number" step="any" bind:value={heightMm} /></label>
      <label>Thickness (mm)<input type="number" step="any" bind:value={thicknessMm} /></label>
      <button type="submit">Create</button>
    </form>
  </section>

  <section class="panel">
    <h2>Recent</h2>
    {#if recents.length === 0}
      <p class="muted">No recent documents.</p>
    {:else}
      <ul class="recents">
        {#each recents as recent (recent.path)}
          <li>
            <button type="button" class="recent" onclick={() => openRecent(recent.path)}>
              <span class="recent-name">{recent.name}</span>
              <span class="recent-path">{recent.path}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</main>

<style>
  :root {
    color: #1f2430;
    background: #f4f5f7;
    font-family: Inter, system-ui, sans-serif;
  }
  .container { max-width: 860px; margin: 0 auto; padding: 32px 20px; }
  .subtitle { color: #5f6b7a; margin-top: -8px; }
  .panel { background: #fff; border: 1px solid #d9dde3; border-radius: 12px; padding: 16px; margin: 16px 0; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; }
  .actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .status { display: grid; grid-template-columns: 140px 1fr; gap: 8px 16px; margin: 12px 0 0; }
  dt { color: #5f6b7a; }
  dd { margin: 0; word-break: break-word; }
  .form { display: grid; gap: 12px; max-width: 320px; }
  label { display: grid; gap: 4px; font-size: 14px; }
  input, button { font: inherit; padding: 8px 10px; border-radius: 8px; border: 1px solid #c7ced8; }
  button { cursor: pointer; background: #1f2430; color: #fff; border-color: #1f2430; }
  .error { color: #b42318; }
  .muted { color: #5f6b7a; }
  .recents { list-style: none; padding: 0; margin: 12px 0 0; display: grid; gap: 8px; }
  .recent { width: 100%; text-align: left; background: #fff; color: #1f2430; border: 1px solid #d9dde3; }
  .recent-name { display: block; font-weight: 600; }
  .recent-path { display: block; font-size: 12px; color: #5f6b7a; word-break: break-all; }
</style>
