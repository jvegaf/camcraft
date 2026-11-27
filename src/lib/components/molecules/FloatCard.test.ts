import { render, screen } from '@testing-library/svelte';
import { panel } from 'styled-system/recipes';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import FloatCard from './FloatCard.svelte';

// Molecule contract (095 T5): `FloatCard` is the floating panel surface the Browser and Inspector
// organisms share, so it lives with the other molecules instead of in the workbench folder. It is a
// surface only: title, optional meta, optional header action and a body slot — no store, no Tauri.
const body = createRawSnippet(() => ({
  render: () => '<p>Contenido del panel</p>'
}));
const action = createRawSnippet(() => ({
  render: () => '<button type="button">Acción del panel</button>'
}));

describe('FloatCard', () => {
  it('renders the titled overlay surface with its body slot', () => {
    const { container } = render(FloatCard, {
      title: 'Browser',
      children: body
    });

    const surface = screen.getByLabelText('Browser');
    expect(surface.tagName).toBe('ASIDE');
    // The artboard's floating card surface: `Overlay` tone and `radius-overlay` (8px after S6).
    expect(surface.getAttribute('class')).toContain(panel({ tone: 'overlay' }));
    expect(
      screen.getByRole('heading', { level: 2, name: 'Browser' })
    ).toBeTruthy();
    expect(screen.getByText('Contenido del panel')).toBeTruthy();
    expect(container.querySelector('aside')).not.toBeNull();
  });

  it('renders the optional meta and header action without renaming the card', () => {
    render(FloatCard, {
      title: 'Inspector',
      meta: 'Drill 0.8 / 1.0',
      headAction: action,
      children: body
    });

    const surface = screen.getByLabelText('Inspector');
    expect(surface.textContent).toContain('Drill 0.8 / 1.0');
    expect(
      screen.getByRole('button', { name: 'Acción del panel' })
    ).toBeTruthy();
    expect(
      screen.getByRole('heading', { level: 2, name: 'Inspector' })
    ).toBeTruthy();
  });

  it('uses its own aria label when the title is not the accessible name', () => {
    render(FloatCard, {
      title: 'Operación',
      ariaLabel: 'Inspector',
      children: body
    });

    expect(screen.getByLabelText('Inspector')).toBeTruthy();
    expect(
      screen.getByRole('heading', { level: 2, name: 'Operación' })
    ).toBeTruthy();
  });
});
