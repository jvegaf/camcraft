<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { css } from 'styled-system/css';
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import BoardCanvas from './BoardCanvas.svelte';
import BrowserPanel from './BrowserPanel.svelte';
import InspectorPanel from './InspectorPanel.svelte';
import StatusBar from './StatusBar.svelte';
import ToolRail from './ToolRail.svelte';
import TopBar from './TopBar.svelte';
import ViewportHud from './ViewportHud.svelte';
import {
  openSettings,
  refreshDocumentStatus,
  refreshRecents,
  setDocument,
  setStatus,
  workbenchState
} from './workbench.svelte';

interface Props {
  projectName?: string;
}

let { projectName = 'documento.cam' }: Props = $props();

onMount(() => {
  void refreshDocumentStatus();
  void refreshRecents();
});

async function handleOpen() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Camcraft document', extensions: ['cmc'] }]
    });
    if (!selected || Array.isArray(selected)) return;
    const status = await invoke<typeof workbenchState.document>(
      'open_document',
      { path: selected }
    );
    setDocument(status);
    setStatus('Documento abierto', 'ok');
    void refreshRecents();
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

async function handleOpenRecent(path: string) {
  try {
    const status = await invoke<typeof workbenchState.document>(
      'open_document',
      { path }
    );
    setDocument(status);
    setStatus('Documento abierto', 'ok');
    void refreshRecents();
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

async function handleSave() {
  try {
    const path = workbenchState.document?.path ?? null;
    const status = await invoke<typeof workbenchState.document>(
      'save_document',
      { path }
    );
    setDocument(status);
    setStatus('Documento guardado', 'ok');
    void refreshRecents();
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

async function handleSaveAs() {
  try {
    const target = await save({
      filters: [{ name: 'Camcraft document', extensions: ['cmc'] }]
    });
    if (!target) return;
    const status = await invoke<typeof workbenchState.document>(
      'save_document',
      { path: target }
    );
    setDocument(status);
    setStatus('Documento guardado', 'ok');
    void refreshRecents();
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

async function handleNew() {
  try {
    const status = await invoke<typeof workbenchState.document>(
      'new_document',
      {
        request: { width: 100, height: 100, thickness: 1.6 }
      }
    );
    setDocument(status);
    setStatus('Nuevo documento', 'ok');
    void refreshRecents();
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

function handleSettings() {
  openSettings();
}

async function handleBack() {
  await goto('/');
}

const shell = css({
  w: '100%',
  h: '100%',
  minH: '600px',
  position: 'relative',
  overflow: 'hidden',
  bg: 'canvas',
  color: 'text',
  fontSize: '13px',
  lineHeight: '1.4',
  fontFamily: 'sans',
  '& button, & input': {
    font: 'inherit',
    color: 'inherit',
    bg: 'none',
    border: 'none'
  },
  '& button': { cursor: 'pointer' }
});
</script>

<div class={shell} role="application" aria-label="Camcraft Floating Studio">
  <BoardCanvas />
  <TopBar
    projectName={workbenchState.document?.path?.split('/').pop() ?? projectName}
    onNew={handleNew}
    onOpen={handleOpen}
    onSave={handleSave}
    onSaveAs={handleSaveAs}
    onSettings={handleSettings}
    onBack={handleBack}
  />
  <ToolRail onOpen={handleOpen} onSave={handleSave} />
  <ViewportHud />
  <BrowserPanel onOpenRecent={handleOpenRecent} />
  <InspectorPanel />
  <StatusBar />
</div>
