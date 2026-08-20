import { defineComponent, h } from 'vue'
import type { Component } from 'vue'

const iconCache = new Map<string, Component>()
const svgCache = new Map<string, string>()

// 将 SVG 字符串包进组件
function wrapSvg(name: string, svg: string): Component {
  return defineComponent({
    name: `Icon_${name}`,
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

// 回退图标
const fallback = defineComponent({
  name: 'Icon_fallback',
  setup(_, { attrs }) {
    return () =>
      h('span', { ...attrs, style: 'display:inline-flex;line-height:0' }, '\u{1F4C4}')
  },
})

let svgModules: Record<string, () => Promise<{ default: string }>> | null = null

function initModules() {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  svgModules = (import.meta as any).glob('../assets/icons/*.svg', {
    query: '?raw',
    import: 'default',
    eager: false,
  }) as Record<string, () => Promise<{ default: string }>>
}

export function getMaterialIcon(name: string): Component {
  const cached = iconCache.get(name)
  if (cached) return cached

  // 先从已加载的缓存取
  const svg = svgCache.get(name)
  if (svg) {
    const comp = wrapSvg(name, svg)
    iconCache.set(name, comp)
    return comp
  }

  // 异步加载
  if (!svgModules) initModules()
  if (svgModules) {
    const key = Object.keys(svgModules).find((k) => k.endsWith(`/${name}.svg`))
    if (key) {
      svgModules[key]().then((mod) => {
        svgCache.set(name, mod.default)
        iconCache.set(name, wrapSvg(name, mod.default))
      })
    }
  }

  // 先返回 fallback，加载完后自动替换
  if (!iconCache.has(name)) {
    iconCache.set(name, fallback)
  }
  return iconCache.get(name)!
}
