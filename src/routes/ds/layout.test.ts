import { afterEach, describe, expect, it, vi } from 'vitest';

// Dev-only guard for the design-system catalog (097 T3, S3). The build is SPA mode
// (`adapter-static` with `fallback: 'index.html'`), so the page-level `load` is what decides whether
// the route exists at all: `pnpm dev` renders it, a production build answers 404.
describe('ds route guard', () => {
  afterEach(() => {
    vi.resetModules();
    vi.doUnmock('$app/environment');
  });

  it('lets the catalog render under pnpm dev', async () => {
    vi.doMock('$app/environment', () => ({ dev: true }));
    const { load } = await import('./+layout');

    expect(() => load()).not.toThrow();
  });

  it('resolves to a 404 outside pnpm dev', async () => {
    vi.doMock('$app/environment', () => ({ dev: false }));
    const { load } = await import('./+layout');

    try {
      load();
      throw new Error('the guard let a production build render the catalog');
    } catch (thrown) {
      expect(thrown).toMatchObject({ status: 404 });
    }
  });

  it('is never prerendered by the static build', async () => {
    vi.doMock('$app/environment', () => ({ dev: false }));
    const layout = await import('./+layout');

    expect(layout.prerender).toBe(false);
    expect(layout.ssr).toBe(false);
  });
});
