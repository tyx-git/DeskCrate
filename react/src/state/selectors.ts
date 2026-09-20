import { useMemo } from 'react'

import { filterTree } from '../lib/tree'
import { useCatalogStore } from './catalogStore'
import { useUiStore } from './uiStore'
import type { AppData } from '../types/catalog'

/**
 * 叠加搜索词之后的目录树。
 *
 * 侧栏主体、底栏统计、键盘导航都要用同一份过滤结果，抽成 hook 避免三处
 * 各自计算导致不一致。
 */
export function useFilteredData(): AppData {
  const data = useCatalogStore((state) => state.data)
  const query = useUiStore((state) => state.query)

  return useMemo(() => filterTree(data, query), [data, query])
}
