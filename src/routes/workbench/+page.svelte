<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import {
  BoardCanvas,
  CamObjectBrowser,
  OperationInspector,
  StatusBar,
  ToolRail,
  TopBar,
  ViewportHud
} from '$lib/components/organisms';
import { WorkbenchTemplate } from '$lib/components/templates';
import {
  openSettings,
  refreshDocumentStatus,
  refreshRecents,
  setDocument,
  setStatus,
  workbenchState
} from '$lib/components/workbench/workbench.svelte';

// Workbench page (101 T2, S2): the body of the old `WorkbenchShell` component, now the route screen
// the `atomic-design` skill asks for ("Route screen wiring data/actions → Page in `src/routes/`").
// Every Tauri handler, the mount-time bridge calls and the document status/recents wiring live here;
// `WorkbenchTemplate` (atomic tier: Template) only supplies the frame, the backdrop and the region
// slots.
//
// The page still declares `projectName` as an optional prop with its historical default so a
// consumer can mount it with a different label without the page hard-coding one.
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
</script>

<!--
  Page controller: every Tauri handler, the mount-time bridge calls and the document status/recents
  wiring stay here. `WorkbenchTemplate` (atomic tier: Template) only supplies the frame, the
  backdrop and the region slots.
-->
<WorkbenchTemplate ariaLabel="Camcraft Floating Studio">
  {#snippet viewport()}
    <BoardCanvas />
  {/snippet}
  {#snippet topBar()}
    <TopBar
      projectName={workbenchState.document?.path?.split('/').pop() ?? projectName}
      onNew={handleNew}
      onOpen={handleOpen}
      onSave={handleSave}
      onSaveAs={handleSaveAs}
      onSettings={handleSettings}
      onBack={handleBack}
    />
  {/snippet}
  {#snippet toolRail()}
    <ToolRail onOpen={handleOpen} />
  {/snippet}
  {#snippet viewportHud()}
    <ViewportHud />
  {/snippet}
  {#snippet browser()}
    <CamObjectBrowser onOpenRecent={handleOpenRecent} />
  {/snippet}
  {#snippet inspector()}
    <OperationInspector />
  {/snippet}
  {#snippet statusBar()}
    <StatusBar />
  {/snippet}
</WorkbenchTemplate>
