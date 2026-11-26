import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';
import type { DocumentStatus } from '../workbench/workbench.svelte';
import { setDocument, workbenchState } from '../workbench/workbench.svelte';
import CamObjectBrowser from './CamObjectBrowser.svelte';

// Organism contract (095 T4, S5 + S7): the Browser reorganises the workbench panels into the
// artboard's CAM-object IA — «Browser», the «Proyecto» row, then «Board», «Setup», «Operaciones»
// and «Salida». Every section is populated from data the app really has (`document_status` plus the
// workbench store); a section without data names what is missing instead of showing sample rows.
const documentStatus: DocumentStatus = {
  schema: 'cam-document-status@1',
  documentSchema: 'camcraft.document/1',
  dirty: false,
  path: '/tmp/controller-board.cmc',
  layerCount: 4,
  toolpathCount: 3,
  boardWidthMm: 100,
  boardHeightMm: 80,
  boardThicknessMm: 1.6
};

const sectionOf = (title: string) =>
  screen
    .getByRole('heading', { level: 3, name: title })
    .closest('section') as HTMLElement;

describe('CamObjectBrowser', () => {
  beforeEach(() => {
    setDocument(null);
    workbenchState.operations = [];
    workbenchState.selectedOperationId = null;
    workbenchState.selectedBoardObjectId = null;
    workbenchState.collapsedSections = [];
  });

  it('renders the Browser header, the project row and the four section headers', () => {
    setDocument(documentStatus);
    render(CamObjectBrowser);

    const browser = screen.getByLabelText('Browser');
    expect(
      within(browser).getByRole('heading', { level: 2, name: 'Browser' })
    ).toBeTruthy();
    expect(within(browser).getByText('Proyecto')).toBeTruthy();
    expect(within(browser).getByText('controller-board.cmc')).toBeTruthy();

    for (const title of ['Board', 'Setup', 'Operaciones', 'Salida']) {
      expect(
        within(browser).getByRole('heading', { level: 3, name: title })
      ).toBeTruthy();
    }
  });

  it('renders the real board values and the real layer and toolpath counts', () => {
    setDocument(documentStatus);
    render(CamObjectBrowser);

    const board = sectionOf('Board');
    expect(within(board).getByText('Tamaño')).toBeTruthy();
    expect(within(board).getByText('100×80 mm')).toBeTruthy();
    expect(within(board).getByText('Espesor')).toBeTruthy();
    expect(within(board).getByText('1.6 mm')).toBeTruthy();
    // `document_status` exposes how many layers the document has, so that real count sits in the
    // section header; the layer names themselves are not exposed and are not invented as rows.
    expect(within(board).getByText('4')).toBeTruthy();

    const operations = sectionOf('Operaciones');
    expect(within(operations).getByText('3')).toBeTruthy();
    expect(
      within(operations).getByText(/3 operaciones sin detalle/)
    ).toBeTruthy();
  });

  it('renders an honest empty state per section when there is no document', () => {
    render(CamObjectBrowser);

    for (const [title, key] of [
      ['Board', 'board'],
      ['Setup', 'setup'],
      ['Operaciones', 'operations'],
      ['Salida', 'output']
    ] as const) {
      const section = sectionOf(title);
      const empty = section.querySelector(`[data-empty="${key}"]`);
      expect(empty).not.toBeNull();
      expect(empty?.textContent?.trim().length ?? 0).toBeGreaterThan(0);
    }

    expect(
      within(sectionOf('Board')).getByText(/Sin documento abierto/)
    ).toBeTruthy();

    // The artboard's sample rows are data, not strings to hardcode (S4/S5).
    for (const sample of [
      'Top Copper .gbr',
      'Bottom Copper .gbr',
      'Drill Hits .drl',
      'Board Outline .gbr',
      'Stock FR-4 1.6mm 100×80',
      'Origen bottom-left X0 Y0',
      '1 · Profile outline',
      '2 · Drill 0.8 / 1.0',
      '3 · Isolation top',
      'Post GRBL .nc'
    ]) {
      expect(screen.queryByText(sample)).toBeNull();
    }
    expect(screen.queryByText('Tamaño')).toBeNull();
  });

  it('collapses a section through the store and keeps its header', async () => {
    setDocument(documentStatus);
    const user = userEvent.setup();
    render(CamObjectBrowser);

    const collapse = screen.getByRole('button', { name: 'Contraer Board' });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');

    await user.click(collapse);

    expect(
      screen.getByRole('heading', { level: 3, name: 'Board' })
    ).toBeTruthy();
    expect(screen.queryByText('Tamaño')).toBeNull();
    expect(workbenchState.collapsedSections).toEqual(['board']);

    const expand = screen.getByRole('button', { name: 'Expandir Board' });
    expect(expand.getAttribute('aria-expanded')).toBe('false');

    await user.click(expand);
    expect(screen.getByText('Tamaño')).toBeTruthy();
    expect(workbenchState.collapsedSections).toEqual([]);
  });

  it('selects a board object through the store without inventing rows', async () => {
    setDocument(documentStatus);
    const user = userEvent.setup();
    render(CamObjectBrowser);

    const row = screen.getByRole('button', { name: /Tamaño/ });
    expect(row.getAttribute('aria-current')).toBeNull();

    await user.click(row);

    expect(workbenchState.selectedBoardObjectId).toBe('size');
    expect(
      screen
        .getByRole('button', { name: /Tamaño/ })
        .getAttribute('aria-current')
    ).toBe('true');
    expect(
      screen
        .getByRole('button', { name: /Espesor/ })
        .getAttribute('aria-current')
    ).toBeNull();
  });
});
