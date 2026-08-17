/** 字节数格式化为人类可读大小，如 "1.2 GB" */
export function formatSize(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let i = 0
  let v = bytes
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

/** 是否含非法 HTTP 头字符（CR/LF，会破坏请求导致 400） */
export function hasUnsafeHeaderChars(value: string): boolean {
  return /[\r\n]/.test(value)
}

/** 清除非法头字符并去除首尾空白 */
export function sanitizeHeaderValue(value: string): string {
  return value.replace(/[\r\n]+/g, '').trim()
}

/** 解析大小字符串为字节数：支持 "100M"、"1G"、"500"、"1.5G"（无单位时视为 MB） */
export function parseSizeText(text: string): number {
  const s = text.trim().toLowerCase()
  const m = /^([\d.]+)\s*([kmgt]?)$/.exec(s)
  if (!m) return 0
  const mult = { k: 1024, m: 1024 ** 2, g: 1024 ** 3, t: 1024 ** 4 }[m[2] || 'm'] as number
  return Number(m[1]) * mult
}

/**
 * Shift 连选：把 list 中 anchorIndex 与 curIndex 之间的项并入当前选中集（按引用去重）。
 * anchor 为最近一次普通点击项的索引（Windows 资源管理器语义：多次 shift 点击均从同一锚点扩展）。
 */
export function expandRangeSelection<T>(selected: T[], list: T[], anchorIndex: number, curIndex: number): T[] {
  const from = Math.min(anchorIndex, curIndex)
  const to = Math.max(anchorIndex, curIndex)
  const merged = selected.slice()
  for (let i = from; i <= to; i++) {
    const item = list[i]
    if (item !== undefined && !merged.includes(item)) merged.push(item)
  }
  return merged
}

/** 是否分片文件名（形如 xxx.001.ct.ke） */
export function isPartName(name: string): boolean {
  return /\.\d+\.\w+\.\w+$/.test(name)
}

/** 从分片文件名解析序号（如 movie.mp4.001.ct.ke -> 1） */
export function parsePartIndex(name: string): number {
  const m = /\.(\d+)\.\w+\.\w+$/.exec(name)
  return m ? Number(m[1]) : 0
}

/** 计算合并文件名：取共同前缀，去掉序号补零部分与尾部 `_ - .` 分隔符 */
export function commonMergedName(names: string[]): string {
  if (!names.length) return ''
  let prefix = names[0]
  for (let i = 1; i < names.length; i++) {
    let end = 0
    while (end < prefix.length && prefix[end] === names[i][end]) end++
    prefix = prefix.slice(0, end)
  }
  let result = prefix
  for (let guard = 0; guard < 4; guard++) {
    const next = result.replace(/0+$/, '').replace(/[_\-.\s]+$/, '')
    if (next === result) break
    result = next
  }
  return result
}
