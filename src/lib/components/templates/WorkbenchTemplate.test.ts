import { render } from '@testing-library/svelte';
import { workbenchTemplate } from 'styled-system/recipes';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import WorkbenchTemplate from './WorkbenchTemplate.svelte';

// Template contract (095 T2, S1 + S4): the frame of
// `design/camcraft-workbench-v2/02-floating-studio.html` — the rounded `.shell` surface with its
// primary border and shadow, plus the `.canvas-bg` grid/radial wash as a layer beneath the
// viewport — and one named region per slot the artboard names. Layout and slots only: no Tauri, no
// document state, so every assertion here is pure DOM.
//
// jsdom computes no layout, so the artboard's pixel geometry (the `.board-svg` inset that lets the
// backdrop read in the frame gutter) is not observable here: the values live with the template
// recipe in `src/lib/ds/templates.ts` and are checked against the artboard when they change. What is
// asserted is what a user can see in the DOM: one frame, one backdrop painted first, and one
// viewport region inside the frame carrying the canvas.
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
  it('renders the floating shell frame for the app application region', () => {
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

  // The viewport is a region of the frame (not a sibling overlay): the canvas it hosts paints above
  // the backdrop and inside the frame the artboard draws the gutter around.
  it('nests a single viewport region inside the frame', () => {
    const { container } = render(WorkbenchTemplate, {
      viewport: snippet(regions.viewport)
    });

    const frame = container.querySelector(
      '[data-region="frame"]'
    ) as HTMLElement;
    const viewportRegions = frame.querySelectorAll('[data-slot="viewport"]');

    expect(viewportRegions).toHaveLength(1);
    expect(viewportRegions[0].getAttribute('class')).toContain(
      workbenchTemplate().viewport
    );
    expect(viewportRegions[0].textContent).toBe(regions.viewport);
    expect(viewportRegions[0].parentElement).toBe(frame);
    expect(frame.querySelectorAll('[data-region="backdrop"]')).toHaveLength(1);
  });
});
