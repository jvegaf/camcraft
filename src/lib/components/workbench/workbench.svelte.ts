export type ViewportSelection = {
  kind: 'triangle';
  index: number;
  avgDepthMm: number;
} | null;

/**
 * Board-space point under the pointer, as published by `BoardCanvas`: `x`/`y` are the board plane
 * coordinates in mm and `z` is the height. `null` means the pointer is not over the canvas, so there
 * is no position to report (095 T3).
 */
export type PointerPosition = { x: number; y: number; z: number } | null;

export type DocumentStatus = {
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

export type StatusKind = 'ok' | 'error' | 'info';

export type RecentProject = {
  name: string;
  path: string;
  lastOpened: string;
};

export type WorkbenchState = {
  selection: ViewportSelection;
  pointer: PointerPosition;
  statusMessage: string;
  statusKind: StatusKind;
  document: DocumentStatus | null;
  recents: RecentProject[];
  settingsOpen: boolean;
};

export const workbenchState: WorkbenchState = $state({
  selection: null,
  pointer: null,
  statusMessage: 'Listo',
  statusKind: 'ok',
  document: null,
  recents: [],
  settingsOpen: false
});

export function setSelection(selection: ViewportSelection) {
  workbenchState.selection = selection;
}

export function setPointerPosition(pointer: PointerPosition) {
  workbenchState.pointer = pointer;
}

export function setStatus(message: string, kind: StatusKind = 'info') {
  workbenchState.statusMessage = message;
  workbenchState.statusKind = kind;
}

export function setStatusMessage(message: string) {
  const kind: StatusKind = message.toLowerCase().startsWith('error')
    ? 'error'
    : 'info';
  setStatus(message, kind);
}

export function setDocument(document: DocumentStatus | null) {
  workbenchState.document = document;
  if (document) {
    workbenchState.statusMessage = document.dirty
      ? 'Cambios sin guardar'
      : 'Listo';
    workbenchState.statusKind = document.dirty ? 'info' : 'ok';
  }
}

export async function refreshDocumentStatus(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const status = await invoke<DocumentStatus>('document_status');
    setDocument(status);
  } catch (e) {
    setStatus(`Error: ${String(e)}`, 'error');
  }
}

export async function refreshRecents(): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const recents = await invoke<RecentProject[]>('list_recents');
    workbenchState.recents = recents;
  } catch {
    // recents are optional in shell
  }
}

export function openSettings() {
  workbenchState.settingsOpen = true;
}

export function closeSettings() {
  workbenchState.settingsOpen = false;
}
