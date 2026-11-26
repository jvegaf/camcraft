// The chrome organisms moved up to the atomic folder in 095 T3; the barrel keeps re-exporting them
// so `$lib/components/workbench` stays the single entry point for the shell's consumers (S8).
export {
  StatusBar,
  ToolRail,
  TopBar,
  ViewportHud
} from '../organisms';
export { default as BoardCanvas } from './BoardCanvas.svelte';
export { default as BrowserPanel } from './BrowserPanel.svelte';
export { default as FloatCard } from './FloatCard.svelte';
export { default as InspectorPanel } from './InspectorPanel.svelte';
export { default as WorkbenchShell } from './WorkbenchShell.svelte';
