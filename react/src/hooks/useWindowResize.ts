// 窗口缩放统一会话：右/顶/底边缘与右下角共用。增量用 screen 坐标（窗口被
// clamp 移动时 viewport 原点变化不污染增量）；实时 resize_window 经 rAF 节流，
// 松手补写被节流的最后一次尺寸并回读配置。几何全部是设计稿 CSS 像素。
import { useCallback, useEffect, useRef, useState } from 'react'

import { backend, describeError, isTauriRuntime, toAppError } from '../api'
import {
  nextResizeGeometry,
  type ResizeEdge,
  type ResizeSession,
  type ResizeWorkArea,
} from '../lib/windowResize'
import { WINDOW_SPEC } from '../spec/windowSpec'
import { useSettingsStore } from '../state/settingsStore'
import { useUiStore } from '../state/uiStore'
import type { WindowGeometry } from '../types/settings'

export type { ResizeEdge }

export type StartResize = (edge: ResizeEdge) => (event: React.PointerEvent) => void

const GUTTER_X = WINDOW_SPEC.sidebarLeftPx + WINDOW_SPEC.overlayShadowGutterPx
const GUTTER_Y = WINDOW_SPEC.overlayShadowGutterPx * 2

interface Session extends ResizeSession {
  screenX: number
  screenY: number
  moved: boolean
}

/** Chromium(WebView2) 提供 availLeft/availTop；标准 DOM 类型里没有这两个字段。 */
interface ScreenWithAvail extends Screen {
  availLeft?: number
  availTop?: number
}

function currentWorkArea(): ResizeWorkArea {
  const screen = window.screen as ScreenWithAvail
  const left = screen.availLeft ?? 0
  const top = screen.availTop ?? 0
  return {
    left,
    top,
    right: left + screen.availWidth,
    bottom: top + screen.availHeight,
  }
}

export function useWindowResize() {
  const [resizing, setResizing] = useState(false)
  const sessionRef = useRef<Session | null>(null)
  const pendingRef = useRef<WindowGeometry | null>(null)
  const failureRef = useRef<unknown>(null)
  const rafRef = useRef(0)

  const flush = useCallback(() => {
    rafRef.current = 0
    const geometry = pendingRef.current
    if (!geometry) return
    void backend.resizeWindow(geometry).catch((error: unknown) => {
      failureRef.current = error
    })
  }, [])

  const onMove = useCallback(
    (e: PointerEvent) => {
      const s = sessionRef.current
      if (!s) return
      const dx = e.screenX - s.screenX
      const dy = e.screenY - s.screenY
      if (dx === 0 && dy === 0) return
      s.moved = true

      const next = nextResizeGeometry(s, dx, dy)
      useUiStore.getState().setSizeBadge(`${next.width - GUTTER_X} × ${next.height - GUTTER_Y}`)
      pendingRef.current = next
      if (!rafRef.current) rafRef.current = requestAnimationFrame(flush)
    },
    [flush],
  )

  const onUp = useCallback(() => {
    const s = sessionRef.current
    if (!s) return
    sessionRef.current = null
    setResizing(false)
    useUiStore.getState().setSizeBadge(null)
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', onUp)
    window.removeEventListener('blur', onUp)
    cancelAnimationFrame(rafRef.current)
    rafRef.current = 0

    const pending = pendingRef.current
    pendingRef.current = null
    const failure = failureRef.current
    failureRef.current = null
    if (!s.moved) return

    void (async () => {
      try {
        if (failure) throw failure
        if (pending) await backend.resizeWindow(pending)
        await backend.finishWindowResize()
        await useSettingsStore.getState().load()
      } catch (error) {
        useUiStore.getState().notify('error', `保存窗口尺寸失败：${describeError(toAppError(error))}`)
      }
    })()
  }, [onMove])

  const start = useCallback<StartResize>(
    (edge) => (e) => {
      if (!isTauriRuntime()) return
      const settings = useSettingsStore.getState()
      const ui = useUiStore.getState()
      if (settings.viewMode === 'collapsed' || settings.stageConverged) return
      if (ui.dialog || ui.ctx || ui.settingsOpen || sessionRef.current) return
      e.preventDefault()
      // 指针捕获让拖出窗口后仍能收到 pointerup/pointercancel；失败也不影响窗口内拖动。
      try {
        e.currentTarget.setPointerCapture(e.pointerId)
      } catch {
        // 某些合成指针事件不支持捕获，保持窗口级监听即可。
      }
      sessionRef.current = {
        edge,
        screenX: e.screenX,
        screenY: e.screenY,
        x: Math.round(window.screenX),
        y: Math.round(window.screenY),
        width: Math.round(window.innerWidth),
        height: Math.round(window.innerHeight),
        workArea: currentWorkArea(),
        moved: false,
      }
      failureRef.current = null
      setResizing(true)
      useUiStore.getState().setSizeBadge(`${window.innerWidth - GUTTER_X} × ${window.innerHeight - GUTTER_Y}`)
      window.addEventListener('pointermove', onMove)
      window.addEventListener('pointerup', onUp, { once: true })
      window.addEventListener('pointercancel', onUp, { once: true })
      window.addEventListener('blur', onUp, { once: true })
    },
    [onMove, onUp],
  )

  useEffect(
    () => () => {
      cancelAnimationFrame(rafRef.current)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('pointercancel', onUp)
      window.removeEventListener('blur', onUp)
    },
    [onMove, onUp],
  )

  return { start, resizing }
}
