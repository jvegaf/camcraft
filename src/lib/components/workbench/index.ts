// The chrome organisms moved up to the atomic folder in 095 T3 and the CAM-content ones in T4; the
// barrel keeps re-exporting them so `$lib/components/workbench` stays the single entry point for the
// shell's consumers (S8).
export {
  CamObjectBrowser,
  OperationInspector,
  StatusBar,
  ToolRail,
  TopBar,
  ViewportHud
} from '../organisms';
export { default as BoardCanvas } from './BoardCanvas.svelte';
export { default as WorkbenchShell } from './WorkbenchShell.svelte';
