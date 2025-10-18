import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    // Listen on all interfaces for remote access
    host: true,
    // Allow custom hostnames when the app is accessed behind a domain
    allowedHosts: [
      'siva.remotesession.com',
      'localhost',
      '127.0.0.1'
    ]
  }
});
