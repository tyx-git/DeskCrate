// React 挂载入口：先把窗口规格注入 CSS 变量，样式与几何共用同一份 JSON。
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import App from './App'
import './index.css'
import { applyWindowSpecCssVariables } from './spec/windowSpec'

applyWindowSpecCssVariables()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)