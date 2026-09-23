import { invoke } from '@tauri-apps/api/core'

import { isTauriRuntime } from '../api/client'
import { COMMANDS } from '../api/commands'

/**
 * 主窗口控制。
 *
 * 这些调用只在 Tauri 里可用，因此使用动态 import：浏览器预览时不会把
 * Tauri 的窗口模块拉进依赖图。收起态导致的窗口缩放由后端
 * `apply_view_mode` 负责，前端不直接接触 setSize/setPosition。
 */

/** 显示主窗口。窗口在 tauri.conf.json 里配置为 `visible: false`，由前端在初始化后显示。 */
export async function showMainWindow(): Promise<void> {
  if (!isTauriRuntime()) {
    return
  }

  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().show()
}

/** 关闭主窗口，触发后端在退出事件里保存窗口状态。 */
export async function closeMainWindow(): Promise<void> {
  if (!isTauriRuntime()) {
    return
  }

  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}

/** 一次窗口几何下发，四个值都是设计稿 CSS 像素。 */
export interface WindowGeometry {
  x: number
  y: number
  width: number
  height: number
}

/**
 * 把窗口摆到指定的几何上。
 *
 * 单位是设计稿 CSS 像素：窗口没有原生缩放边框可用（无边框窗口的
 * `WM_NCHITTEST` 会被铺满客户区的 WebView2 子窗口截走，父窗口收不到非客户区
 * 消息，`startResizeDragging` 也不会触发缩放循环），所以缩放由前端把手驱动，
 * 每次指针移动算一次几何再交给后端换算和夹取。
 *
 * 缩放结果由后端的尺寸监听器按「用户拖拽」记录并节流落盘，因此这里不需要
 * 另外保存 —— 拖出来的尺寸会照常被记住。
 */
export async function applyWindowGeometry(geometry: WindowGeometry): Promise<void> {
  if (!isTauriRuntime()) {
    return
  }

  // 逐字段传而不是整个对象：Tauri 的 `invoke` 要求参数是
  // `Record<string, unknown>`，接口类型没有索引签名，整体传过不去。
  await invoke<void>(COMMANDS.resizeWindow, {
    x: geometry.x,
    y: geometry.y,
    width: geometry.width,
    height: geometry.height,
  })
}

/**
 * 缩放把手松手时补写一次尺寸。
 *
 * 拖拽期间的尺寸是靠 `Resized` 监听器按 300ms 节流落盘的，最后一段位移正好
 * 落在节流窗口里的概率很高。松手后不会再有尺寸变化，这里让后端把暂存的
 * 覆盖值立刻写下去，记住的才是用户最终拖出来的尺寸。
 */
export async function finishWindowResize(): Promise<void> {
  if (!isTauriRuntime()) {
    return
  }

  await invoke<void>(COMMANDS.finishWindowResize)
}

/** 让窗口获得焦点（收起态展开后调用，避免输入焦点留在别处）。 */
export async function focusMainWindow(): Promise<void> {
  if (!isTauriRuntime()) {
    return
  }

  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().setFocus()
}
