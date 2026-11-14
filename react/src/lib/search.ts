// 搜索：按名称/路径过滤，前缀命中优先，再按中文拼音排序。
import { allApps, type AppWithPath } from './tree'
import type { AppData } from '../types/catalog'

export function searchApps(data: AppData, query: string): AppWithPath[] {
  const q = query.trim().toLowerCase()
  if (!q) return []
  const list = allApps(data).filter(
    ({ app }) => app.name.toLowerCase().includes(q) || app.sourcePath.toLowerCase().includes(q),
  )
  list.sort((a, b) => {
    const as = a.app.name.toLowerCase().startsWith(q) ? 0 : 1
    const bs = b.app.name.toLowerCase().startsWith(q) ? 0 : 1
    return as - bs || a.app.name.localeCompare(b.app.name, 'zh')
  })
  return list
}

/** 查询串看起来像一个可添加的来源（网址或本地路径）时，空结果页给出添加入口。 */
export function looksAddable(query: string): boolean {
  const q = query.trim()
  return /^https?:\/\/\S+$/i.test(q) || /^[a-zA-Z]:\\/.test(q) || /\.(exe|lnk)$/i.test(q)
}