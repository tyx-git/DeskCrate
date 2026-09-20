/**
 * 分类/分组折叠状态的本地持久化。
 *
 * 后端 `data.json` 里虽然保存了 `isCollapsed`，但当前 Command 集合中没有
 * 更新折叠状态的接口（`rename_*` 只改名称）。所以前端先把折叠状态作为
 * 纯 UI 偏好存在本地，等后端补上对应 Command 后把这里换成 IPC 调用即可。
 */

const STORAGE_KEY = 'deskcrate.collapsed'

type CollapseMap = Record<string, boolean>

export function readCollapseMap(): CollapseMap {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    return raw ? (JSON.parse(raw) as CollapseMap) : {}
  } catch {
    return {}
  }
}

export function writeCollapse(nodeId: string, isCollapsed: boolean): void {
  const map = readCollapseMap()
  map[nodeId] = isCollapsed

  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
  } catch {
    // 隐私模式等场景下写不进去，只影响下次启动的展开状态。
  }
}
