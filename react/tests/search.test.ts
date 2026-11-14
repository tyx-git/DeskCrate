// 搜索排序与「可添加」判断的规格测试。
import { describe, expect, it } from 'vitest'

import type { AppData, AppEXE } from '../src/types/catalog'
import { looksAddable, searchApps } from '../src/lib/search'

const app = (id: string, name: string, sourcePath: string): AppEXE => ({
  id,
  name,
  sourcePath,
  iconUrl: 'data:x',
  iconType: 'Executable',
  createdTime: '2026-01-01T00:00:00Z',
})

const d: AppData = {
  version: 1,
  items: [
    {
      Apps: [
        app('1', '微信', 'C:\\WeChat\\WeChat.exe'),
        app('2', 'VS Code', 'C:\\VSCode\\Code.exe'),
        app('3', '资源管理器', 'C:\\Windows\\explorer.exe'),
      ],
    },
    {
      Category: {
        id: 'c',
        name: '开发',
        isCollapsed: false,
        createdTime: '2026-01-01T00:00:00Z',
        groups: [{ App: app('4', '终端', 'C:\\Windows\\cmd.exe') }],
      },
    },
  ],
}

describe('searchApps', () => {
  it('空查询返回空列表', () => {
    expect(searchApps(d, '   ')).toEqual([])
  })

  it('按名称子串命中且大小写不敏感', () => {
    expect(searchApps(d, 'code').map((r) => r.app.id)).toEqual(['2'])
    expect(searchApps(d, 'CODE').map((r) => r.app.id)).toEqual(['2'])
  })

  it('按来源路径命中并带上分类面包屑', () => {
    const hits = searchApps(d, 'cmd.exe')
    expect(hits.map((r) => r.app.id)).toEqual(['4'])
    expect(hits[0].path).toEqual(['开发'])
  })

  it('前缀命中排在子串命中之前', () => {
    const data: AppData = {
      version: 1,
      items: [{ Apps: [app('a', 'Git Bash', 'C:\\git.exe'), app('b', 'Digit', 'C:\\digit.exe')] }],
    }
    expect(searchApps(data, 'git').map((r) => r.app.id)).toEqual(['a', 'b'])
  })

  it('中文名称可搜索', () => {
    expect(searchApps(d, '微信').map((r) => r.app.id)).toEqual(['1'])
    expect(searchApps(d, '管理').map((r) => r.app.id)).toEqual(['3'])
  })
})

describe('looksAddable', () => {
  it('网址、盘符路径、exe/lnk 后缀视为可添加', () => {
    expect(looksAddable('https://example.com')).toBe(true)
    expect(looksAddable('http://a.cn/path')).toBe(true)
    expect(looksAddable('C:\\Tools\\app.exe')).toBe(true)
    expect(looksAddable('D:\\link.lnk')).toBe(true)
    expect(looksAddable('tool.EXE')).toBe(true)
  })

  it('普通关键词不算可添加', () => {
    expect(looksAddable('微信')).toBe(false)
    expect(looksAddable('example.com')).toBe(false)
    expect(looksAddable('')).toBe(false)
  })
})