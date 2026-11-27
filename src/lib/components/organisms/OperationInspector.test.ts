import { render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import type { CamOperation } from '../workbench/workbench.svelte';
import {
  selectOperation,
  setOperations,
  setStatus,
  workbenchState
} from '../workbench/workbench.svelte';
import OperationInspector from './OperationInspector.svelte';

// Organism contract (095 T4, S5 + S7): the Inspector shows the artboard's operation fields with its
// exact labels and, for every value the app does not know, an em dash instead of a plausible
// constant. The «Estado» block shows the real workbench status message.
const known: CamOperation = {
  id: 'op-2',
  index: 2,
  name: 'Drill 0.8 / 1.0',
  type: 'Drill cycle G81',
  tool: 'T1 Flat 0.8 mm',
  zCutMm: -1.8,
  plungeFeedMmPerMin: 120,
  hitCount: 6,
  tools: ['T1', 'T2'],
  durationS: 42
};

const unknown: CamOperation = {
  id: 'op-3',
  index: 3,
  name: 'Isolation top',
  type: null,
  tool: null,
  zCutMm: null,
  plungeFeedMmPerMin: null,
  hitCount: null,
  tools: null,
  durationS: null
};

const sectionOf = (title: string) =>
  screen
    .getByRole('heading', { level: 3, name: title })
    .closest('section') as HTMLElement;

describe('OperationInspector', () => {
  beforeEach(() => {
    workbenchState.operations = [];
    workbenchState.selectedOperationId = null;
    workbenchState.collapsedSections = [];
    setStatus('Ready', 'ok');
  });

  it('shows an honest empty state when no operation is selected', () => {
    render(OperationInspector);

    expect(
      screen.getByRole('heading', { level: 2, name: 'Inspector' })
    ).toBeTruthy();

    const operacion = sectionOf('Operation');
    const empty = operacion.querySelector('[data-empty="operation"]');
    expect(empty).not.toBeNull();
    expect(empty?.textContent).toMatch(/No operation selected/);

    // No field is rendered for an operation the app does not have, so no plausible constant either.
    expect(within(operacion).queryByText('Type')).toBeNull();
    expect(within(operacion).queryByText('Drill cycle G81')).toBeNull();
    expect(within(operacion).queryByText('120')).toBeNull();
  });

  it('renders the exact field labels and values of the selected operation', () => {
    setOperations([known]);
    selectOperation('op-2');
    render(OperationInspector);

    const inspector = screen.getByLabelText('Inspector');
    expect(within(inspector).getByText('Drill 0.8 / 1.0')).toBeTruthy();

    for (const label of [
      'Type',
      'Tool',
      'Z cut',
      'Feed plunge',
      'Hits',
      'Tools',
      'Time'
    ]) {
      expect(within(inspector).getByText(label)).toBeTruthy();
    }

    expect(within(inspector).getByText('Drill cycle G81')).toBeTruthy();
    expect(within(inspector).getByText('T1 Flat 0.8 mm')).toBeTruthy();
    expect(within(inspector).getByText('-1.8')).toBeTruthy();
    expect(within(inspector).getByText('120')).toBeTruthy();
    expect(within(inspector).getByText('mm/min')).toBeTruthy();
    expect(within(inspector).getByText('6')).toBeTruthy();
    expect(within(inspector).getByText('T1, T2')).toBeTruthy();
    expect(within(inspector).getByText('0:42')).toBeTruthy();
  });

  // The invariant the panel owes the operator: for an operation whose parameters the app does not
  // have, every labelled value reads as unknown and no plausible number or unit is rendered.
  it('renders no fabricated value for the parameters the operation does not expose', () => {
    setOperations([unknown]);
    selectOperation('op-3');
    render(OperationInspector);

    const inspector = screen.getByLabelText('Inspector');

    const fieldValue = (label: string): string => {
      const labelEl = within(inspector).getByText(label);
      const box = labelEl.parentElement as HTMLElement;
      return (box.textContent ?? '').replace(label, '').trim();
    };
    const rowValue = (label: string): string => {
      const term = within(inspector).getByText(label);
      return term.nextElementSibling?.textContent?.trim() ?? '';
    };

    for (const label of ['Type', 'Tool', 'Z cut', 'Feed plunge']) {
      expect(fieldValue(label), `${label} shows a fabricated value`).toBe('—');
    }
    for (const label of ['Hits', 'Tools', 'Time']) {
      expect(rowValue(label), `${label} shows a fabricated value`).toBe('—');
    }

    // No unit is claimed for a value the app does not have, and none of the artboard's sample
    // parameters leak into the panel.
    expect(within(inspector).queryByText('mm')).toBeNull();
    expect(within(inspector).queryByText('mm/min')).toBeNull();
    expect(within(inspector).queryByText('Drill cycle G81')).toBeNull();
    expect(within(inspector).queryByText('-1.8')).toBeNull();
    expect(within(inspector).queryByText('120')).toBeNull();
    expect(within(inspector).queryByText('0:42')).toBeNull();
  });

  it('renders the real workbench status message in the Status block', async () => {
    setStatus('Valid toolpath, no collisions detected', 'ok');
    render(OperationInspector);

    const estado = sectionOf('Status');
    expect(
      within(estado).getByText('Valid toolpath, no collisions detected')
    ).toBeTruthy();

    setStatus('Error: no document open', 'error');
    await waitFor(() =>
      expect(
        within(sectionOf('Status')).getByText('Error: no document open')
      ).toBeTruthy()
    );
  });
});
