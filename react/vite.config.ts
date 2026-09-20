import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// Tauri 通过 devUrl 固定访问 5173，因此这里显式锁定端口，
// 避免端口被占用时 Vite 自动换端口导致 Tauri 一直等待。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // WebView2 基于 Chromium，可以直接使用现代语法，不必为旧浏览器降级。
    target: 'chrome105',
    sourcemap: false,
  },
})
