// 应用图标渲染：icon:pending 显示脉动地球，其余 iconUrl 直接作为 <img src>。
import type { AppEXE } from '../types/catalog'
import { Glyph } from './Glyph'

export function PlateInner({ app }: { app: AppEXE }) {
  if (app.iconUrl === 'icon:pending') return <Glyph name="globe" />
  return <img src={app.iconUrl} alt="" draggable={false} />
}

export function AppPlate({ app }: { app: AppEXE }) {
  const pending = app.iconUrl === 'icon:pending'
  return (
    <span className={`plate${pending ? ' is-pending' : ''}`}>
      <PlateInner app={app} />
    </span>
  )
}