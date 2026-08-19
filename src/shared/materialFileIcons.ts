import { defineComponent, h } from 'vue'
import type { Component } from 'vue'

// 使用 Vite 的 import.meta.glob 批量加载所有 SVG 为原始字符串
const svgModules = import.meta.glob<string>('../assets/icons/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
})

// 构建图标名 → SVG 内容的映射
const iconSvgs = new Map<string, string>()
for (const [path, svg] of Object.entries(svgModules)) {
  const name = path.split('/').pop()?.replace('.svg', '') ?? ''
  iconSvgs.set(name, svg)
}

// 根据图标名创建 Vue 组件（内联 SVG，保留原始配色）
function createIconComponent(name: string): Component | null {
  const svg = iconSvgs.get(name)
  if (!svg) return null
  return defineComponent({
    name: `MatIcon_${name}`,
    setup(_, { attrs }) {
      return () =>
        h('span', {
          ...attrs,
          class: 'mat-file-icon',
          innerHTML: svg,
          style: 'display:inline-flex;line-height:0',
        })
    },
  })
}

// 预生成常用图标组件，避免重复创建
const cache = new Map<string, Component>()

export function getMaterialIcon(name: string): Component {
  let comp = cache.get(name)
  if (!comp) {
    comp = createIconComponent(name) ?? createIconComponent('document')!
    cache.set(name, comp)
  }
  return comp
}
