import react from '@vitejs/plugin-react'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import mdx from 'fumadocs-mdx/vite'
import { defineConfig } from 'vite'

export default defineConfig({ plugins: [mdx(await import('./source.config')), tanstackStart({ prerender: { enabled: true } }), react()] })
