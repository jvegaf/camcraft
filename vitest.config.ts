import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [sveltekit()],
  resolve: {
    conditions: ['browser']
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{js,ts}'],
    globals: true,
    // A suite that matches no file is not a pass: `passWithNoTests: true` let the gate go green with
    // zero frontend tests, which says nothing about the app. Every run must have something to run.
    passWithNoTests: false
  }
});
