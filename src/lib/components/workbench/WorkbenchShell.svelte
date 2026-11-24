<script lang="ts">
import { css } from 'styled-system/css';
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
  <TopBar {projectName} />
  <ToolRail />
  <ViewportHud />
  <BrowserPanel />
  <InspectorPanel />
  <StatusBar />
</div>
