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

/**
 * The sections of the Browser (095 T4, S5): the artboard's CAM-object IA, in that order. The id is
 * what the store keeps collapsed, so the panel and the state share one closed set.
 */
export type CamSectionId = 'board' | 'setup' | 'operations' | 'output';

/**
 * One operation (toolpath) as the Browser and the Inspector can honestly know it (095 T4, S5).
 *
 * Every value is nullable on purpose: `document_status` only exposes `toolpathCount`
 * (`src-tauri/src/lib.rs:145-156`), and no command returns an operation's name, type, tool, Z cut,
 * plunge feed, hit count, tool list or duration. A value the app does not have stays `null` so the
 * Inspector prints an em dash instead of a plausible-looking constant.
 */
export type CamOperation = {
  id: string;
  /** 1-based position in the document, used by the artboard's "1 · …" row prefix. */
  index: number;
  name: string | null;
  type: string | null;
  tool: string | null;
  zCutMm: number | null;
  plungeFeedMmPerMin: number | null;
  hitCount: number | null;
  tools: string[] | null;
  durationS: number | null;
};

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
  /** Operations the Browser can list; empty until a command exposes them (095 T4). */
  operations: CamOperation[];
  selectedOperationId: string | null;
  selectedBoardObjectId: string | null;
  collapsedSections: CamSectionId[];
};

export const workbenchState: WorkbenchState = $state({
  selection: null,
  pointer: null,
  statusMessage: 'Listo',
  statusKind: 'ok',
  document: null,
  recents: [],
  settingsOpen: false,
  operations: [],
  selectedOperationId: null,
  selectedBoardObjectId: null,
  collapsedSections: []
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

// Browser/Inspector presentation state (095 T4). It is only what the operator picked in the panels:
// no document data is duplicated here, and every setter keeps the panel and the store in one shape.
export function setOperations(operations: CamOperation[]) {
  workbenchState.operations = operations;
  const selected = workbenchState.selectedOperationId;
  if (selected !== null && !operations.some(o => o.id === selected)) {
    workbenchState.selectedOperationId = null;
  }
}

export function selectOperation(id: string | null) {
  workbenchState.selectedOperationId = id;
}

export function selectBoardObject(id: string | null) {
  workbenchState.selectedBoardObjectId = id;
}

export function isSectionCollapsed(id: CamSectionId): boolean {
  return workbenchState.collapsedSections.includes(id);
}

export function toggleSection(id: CamSectionId) {
  workbenchState.collapsedSections = isSectionCollapsed(id)
    ? workbenchState.collapsedSections.filter(section => section !== id)
    : [...workbenchState.collapsedSections, id];
}
