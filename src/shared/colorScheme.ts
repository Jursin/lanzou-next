import { argbFromHex, hexFromArgb, themeFromSourceColor } from '@material/material-color-utilities'

/** 配色方案定义 */
interface ColorSchemeDefinition {
  id: string
  label: string
  seed: string
  /** MCU 调色板生成模式：content 低饱和（用于中性色） */
  variant?: 'source' | 'content'
}

/** 10 套预设配色 */
export const COLOR_SCHEMES: ColorSchemeDefinition[] = [
  { id: 'amber', label: '琥珀金', seed: '#E0A422' },
  { id: 'space', label: '深空蓝', seed: '#4A6CF7' },
  { id: 'mint', label: '薄荷绿', seed: '#10B981' },
  { id: 'rose', label: '玫瑰粉', seed: '#F43F5E' },
  { id: 'aurora', label: '极光紫', seed: '#8B5CF6' },
  { id: 'coral', label: '珊瑚橙', seed: '#F97316' },
  { id: 'glacier', label: '冰川青', seed: '#06B6D4' },
  { id: 'evergreen', label: '常青', seed: '#15803D' },
  { id: 'graphite', label: '石墨灰', seed: '#737373', variant: 'content' },
  { id: 'sakura', label: '樱花', seed: '#EC4899' },
]

export const DEFAULT_COLOR_SCHEME = 'glacier'

const STYLE_ID = 'lanzou-color-scheme'

/** 表面容器色调用中性色调色板计算（MCU 0.4 Scheme 不含这些角色） */
function surfaceRoles(def: ColorSchemeDefinition, dark: boolean): Record<string, string> {
  const theme = themeFromSourceColor(argbFromHex(def.seed))
  const p = theme.palettes.neutral
  const t = (tone: number) => hexFromArgb(p.tone(tone))
  if (dark) {
    return {
      '--m3-surface-container-lowest': t(4),
      '--m3-surface-container-low': t(10),
      '--m3-surface-container': t(12),
      '--m3-surface-container-high': t(17),
      '--m3-surface-container-highest': t(22),
    }
  }
  return {
    '--m3-surface-container-lowest': t(100),
    '--m3-surface-container-low': t(96),
    '--m3-surface-container': t(94),
    '--m3-surface-container-high': t(92),
    '--m3-surface-container-highest': t(90),
  }
}

function schemeVars(def: ColorSchemeDefinition, dark: boolean): Record<string, string> {
  const theme = themeFromSourceColor(argbFromHex(def.seed))
  const s = dark ? theme.schemes.dark : theme.schemes.light
  const h = (v: number) => hexFromArgb(v)
  // content 变体（如石墨灰）：主色系用中性色板，避免 MCU 对灰色种子生成偏色主色
  const content = def.variant === 'content'
  const neutral = theme.palettes.neutral
  const primary = content ? h(neutral.tone(dark ? 80 : 40)) : h(s.primary)
  const primaryContainer = content ? h(neutral.tone(dark ? 30 : 90)) : h(s.primaryContainer)
  return {
    '--m3-primary': primary,
    '--m3-primary-container': primaryContainer,
    '--m3-tertiary-container': h(s.tertiaryContainer),
    '--m3-on-surface': h(s.onSurface),
    '--m3-on-surface-variant': h(s.onSurfaceVariant),
    '--m3-outline': h(s.outline),
    '--m3-outline-variant': h(s.outlineVariant),
    ...surfaceRoles(def, dark),
  }
}

/** 解析配色 id 为定义（未知 id 视为十六进制色） */
function resolveScheme(schemeId: string): ColorSchemeDefinition {
  return COLOR_SCHEMES.find((s) => s.id === schemeId) || { id: schemeId, label: schemeId, seed: schemeId }
}

/** 直接由种子色计算 naive-ui 需要的覆盖色（避免读 CSS 变量的时序问题） */
export function naiveTokens(schemeId: string, dark: boolean) {
  const def = resolveScheme(schemeId)
  const vars = schemeVars(def, dark)
  // 语义色（info/success/warning/error）不随配色变化，从当前主题 CSS 读取
  const read = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#000000'
  return {
    primary: vars['--m3-primary'],
    onSurface: vars['--m3-on-surface'],
    outline: vars['--m3-outline'],
    outlineVariant: vars['--m3-outline-variant'],
    body: vars['--m3-surface-container-low'],
    card: vars['--m3-surface-container-lowest'],
    modal: vars['--m3-surface-container-high'],
    popover: vars['--m3-surface-container-high'],
    info: read('--m3-info'),
    success: read('--m3-success'),
    warning: read('--m3-warning'),
    error: read('--m3-error'),
  }
}

/** 将配色方案生成 M3 CSS 变量并注入全局样式（schemeId 为预设 id，如 amber） */
export function applyColorScheme(schemeId: string) {
  const def = resolveScheme(schemeId)
  const light = schemeVars(def, false)
  const dark = schemeVars(def, true)
  const css = `:root { ${Object.entries(light)
    .map(([k, v]) => `${k}: ${v};`)
    .join('')} }
[data-theme='dark'] { ${Object.entries(dark)
    .map(([k, v]) => `${k}: ${v};`)
    .join('')} }`
  let style = document.getElementById(STYLE_ID) as HTMLStyleElement | null
  if (!style) {
    style = document.createElement('style')
    style.id = STYLE_ID
    document.head.appendChild(style)
  }
  style.textContent = css
}
