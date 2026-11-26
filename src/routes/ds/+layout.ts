import { error } from '@sveltejs/kit';
import { dev } from '$app/environment';

// The design-system catalog is a development tool, not an app feature (097 T3, S3).
//
// The build is SPA mode: `adapter-static` emits one `index.html` fallback and every route is
// resolved in the browser. There is no server that could refuse the request, so this page-level
// guard is what decides the route: under `pnpm dev` the catalog renders, and in a production build
// `load` resolves it to SvelteKit's 404. `prerender = false` keeps the whole subtree out of the
// static output.
export const prerender = false;
export const ssr = false;

export function load() {
  if (!dev) error(404, 'Not found');
}
