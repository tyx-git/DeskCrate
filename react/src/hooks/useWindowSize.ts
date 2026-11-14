// 跟踪 webview 视口尺寸：Tauri 里窗口几何由后端下发，变化经 resize 事件到达。
import { useEffect, useState } from 'react'

export interface ViewportSize {
  width: number
  height: number
}

export function useWindowSize(): ViewportSize {
  const [size, setSize] = useState<ViewportSize>({ width: window.innerWidth, height: window.innerHeight })

  useEffect(() => {
    const onResize = () => setSize({ width: window.innerWidth, height: window.innerHeight })
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  return size
}