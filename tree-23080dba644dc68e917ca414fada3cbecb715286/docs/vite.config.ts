import react from '@vitejs/plugin-react'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import tailwindcss from '@tailwindcss/vite'
import mdx from 'fumadocs-mdx/vite'
import { defineConfig } from 'vite'

const pagesBuild = Bun.env['GITHUB_ACTIONS'] === 'true'

export default defineConfig({
  base: pagesBuild ? '/termrock/' : '/',
  plugins: [
    mdx(),
    tailwindcss(),
    tanstackStart({
      spa: {
        enabled: true,
        prerender: { enabled: true, crawlLinks: true },
      },
      pages: [{ path: '/' }, { path: '/docs' }],
    }),
    react(),
  ],
  resolve: {
    tsconfigPaths: true,
  },
})
