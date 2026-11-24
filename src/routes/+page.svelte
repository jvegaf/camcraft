<script lang="ts">
import { invoke } from '@tauri-apps/api/core';

type DocumentStatus = {
  schema: string;
  documentSchema: string;
  dirty: boolean;
  layerCount: number;
  toolpathCount: number;
  boardWidthMm: number;
  boardHeightMm: number;
  boardThicknessMm: number;
};

// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
let status = $state<DocumentStatus | null>(null);
// biome-ignore lint/correctness/noUnusedVariables: used in Svelte template
let error = $state('');
let widthMm = $state(1219);
let heightMm = $state(2438);
let thicknessMm = $state(18);

async function refreshStatus() {
  error = '';
  try {
    status = await invoke<DocumentStatus>('document_status');
  } catch (e) {
    error = String(e);
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
    error = String(e);
  }
}

void refreshStatus();
</script>

<main class="container">
  <h1>Camcraft</h1>
  <p class="subtitle">Tauri host + domain Rust spike</p>

  <section class="panel">
    <div class="row">
      <h2>Document</h2>
      <button type="button" onclick={refreshStatus}>Refresh</button>
    </div>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    {#if status}
      <dl class="status">
        <dt>Schema</dt><dd>{status.documentSchema}</dd>
        <dt>Dirty</dt><dd>{status.dirty ? "yes" : "no"}</dd>
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
</main>

<style>
  :root {
    color: #1f2430;
    background: #f4f5f7;
    font-family: Inter, system-ui, sans-serif;
  }
  .container { max-width: 720px; margin: 0 auto; padding: 32px 20px; }
  .subtitle { color: #5f6b7a; margin-top: -8px; }
  .panel { background: #fff; border: 1px solid #d9dde3; border-radius: 12px; padding: 16px; margin: 16px 0; }
  .row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .status { display: grid; grid-template-columns: 140px 1fr; gap: 8px 16px; margin: 12px 0 0; }
  dt { color: #5f6b7a; }
  dd { margin: 0; }
  .form { display: grid; gap: 12px; max-width: 320px; }
  label { display: grid; gap: 4px; font-size: 14px; }
  input, button { font: inherit; padding: 8px 10px; border-radius: 8px; border: 1px solid #c7ced8; }
  button { cursor: pointer; background: #1f2430; color: #fff; border-color: #1f2430; }
  .error { color: #b42318; }
</style>
