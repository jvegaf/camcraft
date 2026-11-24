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
    --bg-app:#12151a; --bg-canvas:#0d1014; --bg-panel:#171b21; --bg-raised:#1e242b; --bg-overlay:#232a32;
    --line-hair:#262d36; --line-strong:#35404c;
    --text-hi:#e7ecf3; --text:#c3ccd8; --text-dim:#8b95a3; --text-off:#5c6673;
    --sel:#6fd3e8; --sel-soft:rgba(111,211,232,.13);
    --cut:#f5a623; --cut-soft:rgba(245,166,35,.13);
    --ok:#5ec27f; --warn:#f0b429; --danger:#f2635f;
    --radius-control:4px; --radius-panel:6px; --radius-overlay:8px; --radius-dialog:10px;
    --shadow-float:0 1px 0 rgba(255,255,255,.03), 0 12px 32px rgba(0,0,0,.42);
    --font: Inter, "Inter UI", ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    --mono: ui-monospace, SFMono-Regular, Menlo, monospace;
    width:100%; height:100%; min-height:600px; position:relative; overflow:hidden; background:var(--bg-canvas); color:var(--text); font:13px/1.4 var(--font);
  }
  .shell :global(button), .shell :global(input) { font:inherit; color:inherit; background:none; border:none; }
  .shell :global(button) { cursor:pointer; }
</style>
