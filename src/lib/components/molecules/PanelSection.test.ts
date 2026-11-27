import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import PanelSection from './PanelSection.svelte';

const rows = (text: string) =>
  createRawSnippet(() => ({ render: () => `<p>${text}</p>` }));

// Molecule contract: the browser's Board / Setup / Operaciones / Salida block. Header carries the
// section name, an optional count and an optional right slot; the body is a slot.
describe('PanelSection', () => {
  it('renders the section name as a level-3 heading with its body', () => {
    render(PanelSection, { title: 'Board', children: rows('Top Copper') });

    const heading = screen.getByRole('heading', { level: 3, name: 'Board' });
    expect(heading.textContent).toBe('Board');
    expect(screen.getByText('Top Copper')).toBeTruthy();
  });

  it('renders an optional count next to the heading without renaming it', () => {
    render(PanelSection, {
      title: 'Operations',
      count: 3,
      children: rows('1 · Profile outline')
    });

    expect(
      screen.getByRole('heading', { level: 3, name: 'Operations' })
    ).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
  });

  it('renders an optional right slot in the header', () => {
    render(PanelSection, {
      title: 'Browser',
      count: 'Project',
      right: createRawSnippet(() => ({
        render: () => `<button type="button">Search in browser</button>`
      })),
      children: rows('Top Copper')
    });

    expect(
      screen.getByRole('button', { name: 'Search in browser' })
    ).toBeTruthy();
    expect(screen.getByText('Project')).toBeTruthy();
  });
});
