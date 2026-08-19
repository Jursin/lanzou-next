import type { Component } from 'vue'
import { getMaterialIcon } from './materialFileIcons'

// 蓝奏云支持的扩展名 → Material Icon 图标名
const iconMap: Record<string, string> = {
  // word
  doc: 'word', docx: 'word',
  // zip
  zip: 'zip', rar: 'zip', '7z': 'zip', tar: 'zip', deb: 'zip', rp: 'zip', rpm: 'zip', e: 'zip', z: 'zip', pkg: 'zip',
  // android
  apk: 'android', xapk: 'android',
  // document
  txt: 'document', ct: 'document', ke: 'document', cetrainer: 'document', w3x: 'document',
  osk: 'document', osz: 'document', xpa: 'document', cpk: 'document',
  lolgezi: 'document', bds: 'document', bdi: 'document', ssf: 'document',
  it: 'document', enc: 'document', ce: 'document', rplib: 'document',
  xmind: 'document', brushset: 'document', ipa: 'document',
  imazingapp: 'document', mobileconfig: 'document', crx: 'document',
  // exe
  exe: 'exe', dll: 'exe',
  // database
  db: 'database', accdb: 'database',
  // pdf
  pdf: 'pdf',
  // lua
  lua: 'lua',
  // jar
  jar: 'jar',
  // disc
  dmg: 'disc', iso: 'disc', img: 'disc', gho: 'disc', appimage: 'disc',
  // powerpoint
  ppt: 'powerpoint', pptx: 'powerpoint',
  // table
  xls: 'table', xlsx: 'table',
  // audio
  mp3: 'audio', flac: 'audio',
  // font
  ttf: 'font', ttc: 'font', txf: 'font',
  // 3d
  dwg: '3d', cad: '3d', hwt: '3d',
  // console
  bat: 'console',
  // settings
  cfg: 'settings', conf: 'settings',
  // epub
  epub: 'epub', mobi: 'epub', azw: 'epub', azw3: 'epub',
  // video
  mp4: 'video', avi: 'video',
  // image
  png: 'image', jpeg: 'image', jpg: 'image', gif: 'image', webp: 'image',
}

export function getFileIconComponent(filename: string): Component {
  const ext = getExtension(filename).toLowerCase()
  if (!ext) return getMaterialIcon('document')
  return getMaterialIcon(iconMap[ext] ?? 'document')
}

const colorMap: Record<string, string> = {
  image: 'var(--m3-warning)',
  video: 'var(--m3-primary-container)',
  audio: '#e91e63',
  word: 'color-mix(in srgb, var(--m3-primary) 78%, black)',
  table: '#2e7d32',
  powerpoint: '#ef6c00',
  disc: '#546e7a',
  zip: 'var(--m3-info)',
  epub: 'var(--m3-tertiary-container)',
  '3d': '#0d47a1',
  font: 'var(--m3-outline)',
  android: '#388e3c',
  exe: 'var(--m3-on-surface-variant)',
  database: 'var(--m3-outline-variant)',
  pdf: 'var(--m3-error)',
  document: 'var(--m3-on-surface-variant)',
  console: 'var(--m3-on-surface-variant)',
  jar: '#ff6d00',
  lua: '#3572A5',
  settings: '#78909c',
}

export function getFileIconColor(filename: string): string {
  const ext = getExtension(filename).toLowerCase()
  if (!ext) return 'var(--m3-on-surface-variant)'
  const iconName = iconMap[ext]
  if (iconName && colorMap[iconName]) return colorMap[iconName]
  return 'var(--m3-on-surface-variant)'
}

function getExtension(filename: string): string {
  const dot = filename.lastIndexOf('.')
  return dot === -1 ? '' : filename.slice(dot + 1)
}
