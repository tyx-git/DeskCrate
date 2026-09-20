import { useLayoutEffect, useRef } from 'react'

/**
 * 列表重排时的位移动画（FLIP）。
 *
 * 顺序变了以后，DOM 会瞬间跳到新位置；没有动画的话用户根本看不出「刚才那个
 * 图标去哪了」。这里在每次提交后比对同一批元素的新旧位置，把它们从旧位置
 * 「补」回新位置 —— 视觉上就是平滑地滑过去。
 *
 * 只在**顺序真的变了**时才动画：比对用的是元素 id，位移小于 1px 的（比如仅仅是
 * 重渲染）直接跳过，避免每帧都触发一次动画。
 */
export function useReorderAnimation(signature: string) {
  const previous = useRef(new Map<string, DOMRect>())

  useLayoutEffect(() => {
    const nodes = Array.from(
      document.querySelectorAll<HTMLElement>('[data-app-id]'),
    )

    const current = new Map<string, DOMRect>()

    for (const node of nodes) {
      const id = node.dataset.appId
      if (!id) continue

      const next = node.getBoundingClientRect()
      current.set(id, next)

      const before = previous.current.get(id)
      if (!before) continue

      const dx = before.left - next.left
      const dy = before.top - next.top
      if (Math.abs(dx) < 1 && Math.abs(dy) < 1) continue

      node.animate(
        [
          { transform: `translate(${dx}px, ${dy}px)` },
          { transform: 'none' },
        ],
        {
          duration: 180,
          easing: 'cubic-bezier(0.33, 0, 0.2, 1)',
        },
      )
    }

    previous.current = current
  }, [signature])
}
