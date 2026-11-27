// Organisms (095 T3 + T4, 101 T1): the chrome and CAM-content blocks of the floating studio. Each
// one owns its own local UI state or reads the workbench store, composes atoms/molecules, and keeps
// the props it had in the hand-rolled `workbench/` folder so pages and routes keep working (S8).
//
// `SettingsDialog`, `BoardCanvas` and `StartScreen` moved here from their feature folders (101 T1):
// the layer is the directory the file lives in, so the board counts them as organisms because of
// where they are, never because a manifest claims it (S3).
export { default as BoardCanvas } from './BoardCanvas.svelte';
export { default as CamObjectBrowser } from './CamObjectBrowser.svelte';
export { default as OperationInspector } from './OperationInspector.svelte';
export { default as SettingsDialog } from './SettingsDialog.svelte';
export { default as StartScreen } from './StartScreen.svelte';
export { default as StatusBar } from './StatusBar.svelte';
export { default as ToolRail } from './ToolRail.svelte';
export { default as TopBar } from './TopBar.svelte';
export { default as ViewportHud } from './ViewportHud.svelte';
