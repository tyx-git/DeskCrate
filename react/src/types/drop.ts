// 拖拽落点：前端只描述命中的 DOM 目标，具体如何移动由后端计算。
export type DropSide = 'before' | 'after'

export type DropTarget =
  | { type: 'app'; id: string; side: DropSide }
  | { type: 'container'; id: string; side?: DropSide }
  | { type: 'root'; index: number; side: DropSide }
  | { type: 'rootEnd' }
