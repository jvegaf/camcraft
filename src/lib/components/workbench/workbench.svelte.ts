export type ViewportSelection = {
  kind: 'triangle';
  index: number;
  avgDepthMm: number;
} | null;

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

export type WorkbenchState = {
  selection: ViewportSelection;
  statusMessage: string;
  statusKind: StatusKind;
  document: DocumentStatus | null;
};

export const workbenchState: WorkbenchState = $state({
  selection: null,
  statusMessage: 'Listo',
  statusKind: 'ok',
  document: null
});

export function setSelection(selection: ViewportSelection) {
  workbenchState.selection = selection;
}

export function setStatus(message: string, kind: StatusKind = 'info') {
  workbenchState.statusMessage = message;
  workbenchState.statusKind = kind;
}

export function setStatusMessage(message: string) {
  const kind: StatusKind = message.toLowerCase().startsWith('error') ? 'error' : 'info';
  setStatus(message, kind);
}

export function setDocument(document: DocumentStatus | null) {
  workbenchState.document = document;
  if (document) {
    workbenchState.statusMessage = document.dirty ? 'Cambios sin guardar' : 'Listo';
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
