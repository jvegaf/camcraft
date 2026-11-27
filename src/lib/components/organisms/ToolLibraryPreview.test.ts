import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ToolLibraryPreview from './ToolLibraryPreview.svelte';

// Organism contract (107 T7, S11): the `Preview` column of
// `design/camcraft-workbench-v2/05-settings-tool-library.html`, fed by the `tool_detail` command's
// payload (`src-tauri/src/lib.rs:886-923`). The payload below is that command's JSON, key for key:
// `schema`, `id`, `name`, `kind`, `kindLabel`, the geometry (`number`, `diameterMm`, `flutes`,
// `cutLengthMm`, `shankMm`, `overallLengthMm`, `includedAngleDeg`, `flatDiameterMm`), the feeds and
// `stepoverPercent`, which the model does not hold and T1 publishes as `null`.
//
// The drawing's whole point is the owner correction `jn6b7oo5`: the grey shank is the 3.175 mm
// body and the ochre cutting zone is the 1 mm cutter, so at the same scale the shank must be
// genuinely wider. The assertions below read the rendered `width`/`height` geometry (the same
// units-per-mm scale on both axes) rather than a fill or a class name, so a cosmetically coloured
// but wrongly proportioned drawing fails.
const detail = {
  schema: 'tool-detail@1',
  id: 'flat-22',
  name: 'End mill 1.0 mm',
  kind: 'flat-end-mill',
  kindLabel: 'Flat end mill',
  number: 22,
  diameterMm: 1.0,
  flutes: 1,
  cutLengthMm: 8.0,
  shankMm: 3.175,
  overallLengthMm: 38.0,
  includedAngleDeg: null,
  flatDiameterMm: null,
  spindleRpm: 18000,
  cutFeedMmPerMin: 800,
  plungeFeedMmPerMin: 200,
  stepoverMm: 0.4,
  stepoverPercent: null
};

const partOf = (container: HTMLElement, name: string): SVGElement => {
  const element = container.querySelector(`[data-part="${name}"]`);
  if (!(element instanceof SVGElement)) {
    throw new Error(`no drawing part rendered for ${name}`);
  }
  return element;
};

const metaOf = (container: HTMLElement, name: string): HTMLElement => {
  const element = container.querySelector(`[data-meta="${name}"]`);
  if (!(element instanceof HTMLElement)) {
    throw new Error(`no metadata row rendered for ${name}`);
  }
  return element;
};

const numberOf = (element: Element, attribute: string): number => {
  const raw = element.getAttribute(attribute);
  if (raw === null) {
    throw new Error(
      `no ${attribute} attribute on the ${element.getAttribute('data-part')}`
    );
  }
  return Number(raw);
};

const textOf = (element: Element): string =>
  (element.textContent ?? '').replace(/\s+/g, ' ').trim();

describe('ToolLibraryPreview', () => {
  it('renders the Preview region and a drawing named after the selected tool cutting zone', () => {
    render(ToolLibraryPreview, { detail });

    const preview = screen.getByRole('region', { name: 'Preview' });
    expect(preview).toBeTruthy();
    // The artboard's own accessible name for `role="img"`, with the kind read from the payload.
    expect(
      screen.getByRole('img', {
        name: 'Zoomed view of the selected flat end mill cutting zone'
      })
    ).toBeTruthy();
  });

  it('renders the five S11 metadata rows with the payload values', () => {
    const { container } = render(ToolLibraryPreview, { detail });

    for (const label of [
      'Type:',
      'Diameter:',
      'Shank:',
      'Cut:',
      'Typical use:'
    ]) {
      expect(screen.getByText(label)).toBeTruthy();
    }

    // The type's shape word is not in the payload (T1 publishes no shape family), so it is the
    // app's dash, never an invented `flat`.
    expect(textOf(metaOf(container, 'type'))).toBe('Type: Flat end mill · —');
    expect(textOf(metaOf(container, 'diameter'))).toBe('Diameter: 1.0 mm');
    expect(textOf(metaOf(container, 'shank'))).toBe('Shank: 3.175 mm');
    expect(textOf(metaOf(container, 'cut'))).toBe('Cut: 8.0 mm');
    expect(textOf(metaOf(container, 'typical-use'))).toBe(
      'Typical use: contour and isolation on PCB.'
    );
  });

  it('renders the cut-length 8 and the diameter Ø1 callouts', () => {
    const { container } = render(ToolLibraryPreview, { detail });

    const callouts = [...container.querySelectorAll('svg text')].map(textOf);
    expect(callouts).toContain('8');
    expect(callouts).toContain('Ø1');
  });

  it('draws the shank wider than the cutting zone from the payload values', () => {
    const { container } = render(ToolLibraryPreview, { detail });

    const shank = partOf(container, 'shank');
    const cut = partOf(container, 'cutting-zone');
    const shankWidth = numberOf(shank, 'width');
    const cutWidth = numberOf(cut, 'width');
    const cutHeight = numberOf(cut, 'height');

    expect(shankWidth).toBeGreaterThan(cutWidth);
    // Same scale on both axes: the shank/diameter width ratio is the real 3.175/1, and the
    // cutting zone's height/width ratio is the real cut length over diameter, 8/1.
    expect(shankWidth / cutWidth).toBeCloseTo(3.175 / 1.0, 6);
    expect(cutHeight / cutWidth).toBeCloseTo(8.0 / 1.0, 6);
  });

  it('renders the dash instead of a drawing when the library holds no cutting geometry', () => {
    const { container } = render(ToolLibraryPreview, {
      detail: { ...detail, diameterMm: null, cutLengthMm: null, shankMm: null }
    });

    expect(container.querySelector('svg')).toBeNull();
    const fallback = container.querySelector('[data-preview-fallback]');
    expect(fallback?.textContent?.trim()).toBe('—');
  });
});
