import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// Tauri 通过 devUrl 固定访问 5173，端口被占用时直接报错而不是自动换端口，
// 否则 Tauri 会一直等待一个不存在的开发服务器。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    // WebView2 基于 Chromium，直接使用现代语法，不为旧浏览器降级。
    target: 'chrome105',
    sourcemap: false,
  },
  test: {
    include: ['tests/**/*.test.ts'],
  },
})