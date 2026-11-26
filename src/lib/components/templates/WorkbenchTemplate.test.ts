import { render } from '@testing-library/svelte';
import { workbenchTemplate } from 'styled-system/recipes';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { workbenchTemplateRecipe } from '$lib/ds/templates';
import WorkbenchTemplate from './WorkbenchTemplate.svelte';

// Template contract (095 T2, S1 + S4): the frame of
// `design/camcraft-workbench-v2/02-floating-studio.html` — the rounded `.shell` surface with its
// primary border and shadow, plus the `.canvas-bg` grid/radial wash as a layer beneath the
// viewport — and one named region per slot the artboard names. Layout and slots only: no Tauri, no
// document state, so every assertion here is pure DOM.
const snippet = (text: string) =>
  createRawSnippet(() => ({ render: () => `<span>${text}</span>` }));

const regions = {
  topBar: 'top bar region',
  toolRail: 'tool rail region',
  viewport: 'viewport region',
  viewportHud: 'hud region',
  browser: 'browser region',
  inspector: 'inspector region',
  statusBar: 'status bar region'
} as const;

describe('WorkbenchTemplate', () => {
  it('renders the floating shell frame with the template root surface', () => {
    const { container } = render(WorkbenchTemplate, {});

    const frame = container.querySelector('[data-region="frame"]');
    expect(frame).not.toBeNull();
    expect(frame?.getAttribute('role')).toBe('application');
    expect(frame?.getAttribute('aria-label')).toBe('Camcraft Floating Studio');
    expect(frame?.getAttribute('class')).toContain(workbenchTemplate().root);
  });

  it('renders the grid/radial backdrop beneath the viewport', () => {
    const { container } = render(WorkbenchTemplate, {
      viewport: snippet(regions.viewport)
    });

    const backdrop = container.querySelector(
      '[data-region="backdrop"]'
    ) as HTMLElement;
    const viewport = container.querySelector(
      '[data-slot="viewport"]'
    ) as HTMLElement;

    expect(backdrop).not.toBeNull();
    expect(backdrop.getAttribute('aria-hidden')).toBe('true');
    expect(backdrop.getAttribute('class')).toContain(
      workbenchTemplate().backdrop
    );
    expect(viewport).not.toBeNull();
    // Painter's order is DOM order here: the backdrop must come first so the viewport sits on top.
    expect(
      backdrop.compareDocumentPosition(viewport) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
  });

  it('renders the content of every named region slot', () => {
    const { container } = render(WorkbenchTemplate, {
      topBar: snippet(regions.topBar),
      toolRail: snippet(regions.toolRail),
      browser: snippet(regions.browser),
      inspector: snippet(regions.inspector),
      viewport: snippet(regions.viewport),
      viewportHud: snippet(regions.viewportHud),
      statusBar: snippet(regions.statusBar)
    });

    for (const [slot, text] of Object.entries(regions)) {
      const region = container.querySelector(`[data-slot="${slot}"]`);
      expect(region, `missing region slot: ${slot}`).not.toBeNull();
      expect(region?.textContent).toBe(text);
    }
  });

  it('keeps the frame when a region has no content', () => {
    const { container } = render(WorkbenchTemplate, {});

    expect(container.querySelectorAll('[data-slot]')).toHaveLength(7);
    for (const region of container.querySelectorAll('[data-slot]')) {
      expect(region.textContent).toBe('');
    }
  });

  // 095 T3 item 6: the viewport is inset like the artboard's `.board-svg` (inset 52px 0 34px 0 plus
  // the 34px side gutter of the `.shell` frame), so the gridded backdrop reads in the gutter instead
  // of being covered by an opaque canvas. jsdom computes no layout, so the assertion is the recipe
  // the region is styled with, plus the class the template really applies to the slot.
  it('insets the viewport region to the artboard board proportions', () => {
    const { container } = render(WorkbenchTemplate, {
      viewport: snippet(regions.viewport)
    });

    const viewport = container.querySelector(
      '[data-slot="viewport"]'
    ) as HTMLElement;
    expect(viewport.getAttribute('class')).toContain(
      workbenchTemplate().viewport
    );

    const base = (
      workbenchTemplateRecipe as unknown as {
        base: Record<string, Record<string, unknown>>;
      }
    ).base;
    expect(base.viewport).toMatchObject({
      position: 'absolute',
      top: '52px',
      right: '34px',
      bottom: '34px',
      left: '34px'
    });
  });
});
