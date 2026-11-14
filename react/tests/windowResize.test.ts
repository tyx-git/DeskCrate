import { describe, expect, it } from 'vitest'

import { nextResizeGeometry, type ResizeEdge, type ResizeSession } from '../src/lib/windowResize'

const workArea = { left: 0, top: 0, right: 1920, bottom: 1080 }

function session(edge: ResizeEdge, overrides: Partial<ResizeSession> = {}): ResizeSession {
  return {
    edge,
    x: 100,
    y: 100,
    width: 300,
    height: 500,
    workArea,
    ...overrides,
  }
}

describe('nextResizeGeometry', () => {
  it('e 边只改宽度，x/y 不变', () => {
    expect(nextResizeGeometry(session('e'), 40, 60)).toEqual({
      x: 100,
      y: 100,
      width: 340,
      height: 500,
    })
  })

  it('e 边不会小于最小宽度', () => {
    expect(nextResizeGeometry(session('e'), -1000, 0).width).toBe(220)
  })

  it('s 边只改高度，x/y 不变', () => {
    expect(nextResizeGeometry(session('s'), 40, 60)).toEqual({
      x: 100,
      y: 100,
      width: 300,
      height: 560,
    })
  })

  it('s 边不会大于最大高度', () => {
    expect(nextResizeGeometry(session('s'), 0, 1000).height).toBe(900)
  })

  it('n 边上沿下移，底边不动', () => {
    const next = nextResizeGeometry(session('n'), 0, 40)
    expect(next.y).toBe(140)
    expect(next.height).toBe(460)
    expect(next.y + next.height).toBe(600)
  })

  it('n 边上沿拉过工作区顶时夹在顶边，底边仍不动', () => {
    const next = nextResizeGeometry(session('n', { y: 100, height: 500, workArea: { ...workArea, top: 50 } }), 0, -200)
    expect(next.y).toBe(50)
    expect(next.height).toBe(550)
    expect(next.y + next.height).toBe(600)
  })

  it('n 边向上拉到最大高度后底边仍不动', () => {
    const next = nextResizeGeometry(session('n'), 0, -1000)
    expect(next.y).toBe(0)
    expect(next.height).toBe(600)
    expect(next.y + next.height).toBe(600)
  })

  it('n 边向下拉不会小于最小高度', () => {
    const next = nextResizeGeometry(session('n'), 0, 1000)
    expect(next.height).toBe(320)
    expect(next.y).toBe(280)
    expect(next.y + next.height).toBe(600)
  })

  it('se 角同时改宽高', () => {
    expect(nextResizeGeometry(session('se'), 40, 40)).toEqual({
      x: 100,
      y: 100,
      width: 340,
      height: 540,
    })
  })

  it('e 边夹在窗口工作区右缘', () => {
    const next = nextResizeGeometry(session('e', { x: 1600, width: 300 }), 100, 0)
    expect(next.width).toBe(320)
  })

  it('s 边夹在窗口工作区底缘', () => {
    const next = nextResizeGeometry(session('s', { y: 700, height: 300 }), 0, 100)
    expect(next.height).toBe(380)
  })
})
