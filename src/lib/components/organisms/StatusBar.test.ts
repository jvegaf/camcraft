import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  setDocument,
  setPointerPosition,
  setSelection,
  setStatus
} from '../workbench/workbench.svelte';
import StatusBar from './StatusBar.svelte';

// Organism contract (095 T3, S4 + S6 follow-up). Reference:
// `design/camcraft-workbench-v2/02-floating-studio.html` `.status` — X/Y/Z in mono, the units block
// and the health indicator. The artboard coordinates are sample data, so they are rendered from the
// real pointer position published by `BoardCanvas`; with no pointer over the canvas the cells show
// an em dash instead of a made-up number.
//
// The bar also carries the viewport selection readout it had before the chrome rewrite: the triangle
// index and the average depth of what `BoardCanvas` selected. Dropping it lost information the
// operator could see, so it is part of the contract next to the artboard labels.
describe('StatusBar', () => {
  beforeEach(() => {
    setPointerPosition(null);
    setSelection(null);
    setStatus('Listo', 'ok');
    setDocument(null);
  });

  it('shows em dashes instead of inventing coordinates when the pointer is off the canvas', () => {
    render(StatusBar, {});

    expect(screen.getByText('X —')).toBeTruthy();
    expect(screen.getByText('Y —')).toBeTruthy();
    expect(screen.getByText('Z —')).toBeTruthy();
  });

  it('renders the real pointer position in the three mono cells', () => {
    setPointerPosition({ x: 42.8, y: 31.2, z: 2 });
    render(StatusBar, {});

    expect(screen.getByText('X 42.80')).toBeTruthy();
    expect(screen.getByText('Y 31.20')).toBeTruthy();
    expect(screen.getByText('Z 2.00')).toBeTruthy();
    expect(screen.queryByText('X —')).toBeNull();
  });

  it('renders the artboard units block and the health indicator', () => {
    render(StatusBar, {});

    expect(screen.getByText('mm · Grid 1 mm · GRBL')).toBeTruthy();
    expect(screen.getByText('● Sin errores')).toBeTruthy();
  });

  it('reads out the selected triangle index and its average depth', () => {
    setSelection({ kind: 'triangle', index: 3, avgDepthMm: 0.4 });
    render(StatusBar, {});

    expect(screen.getByText('Tri 3')).toBeTruthy();
    expect(screen.getByText('0.40 mm')).toBeTruthy();
    // The artboard labels the bar owns stay: coordinates, units and health.
    expect(screen.getByText('X —')).toBeTruthy();
    expect(screen.getByText('mm · Grid 1 mm · GRBL')).toBeTruthy();
    expect(screen.getByText('● Sin errores')).toBeTruthy();
  });

  it('shows no selection readout while nothing is selected', () => {
    render(StatusBar, {});

    expect(screen.queryByText(/^Tri /)).toBeNull();
    expect(screen.queryByText(/mm$/)).toBeNull();
  });

  it('keeps the status message and drops the health claim when the app reports an error', () => {
    setStatus('Error: no project open', 'error');
    render(StatusBar, {});

    expect(screen.getByText('Error: no project open')).toBeTruthy();
    expect(screen.queryByText('● Sin errores')).toBeNull();
  });
});
