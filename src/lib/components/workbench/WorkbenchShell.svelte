<script lang="ts">
import BoardCanvas from './BoardCanvas.svelte';
import BrowserPanel from './BrowserPanel.svelte';
import InspectorPanel from './InspectorPanel.svelte';
import StatusBar from './StatusBar.svelte';
import ToolRail from './ToolRail.svelte';
import TopBar from './TopBar.svelte';
import ViewportHud from './ViewportHud.svelte';

interface Props {
  projectName?: string;
}

let { projectName = 'controller-board.cam' }: Props = $props();
</script>

<div class="shell" role="application" aria-label="Camcraft Floating Studio">
  <BoardCanvas />
  <TopBar {projectName} />
  <ToolRail />
  <ViewportHud />
  <BrowserPanel />
  <InspectorPanel />
  <StatusBar />
</div>

<style>
  .shell {
    width:100%; height:100%; min-height:600px; position:relative; overflow:hidden; background:var(--bg-canvas); color:var(--text); font:13px/1.4 var(--font);
  }
  .shell :global(button), .shell :global(input) { font:inherit; color:inherit; background:none; border:none; }
  .shell :global(button) { cursor:pointer; }
</style>
