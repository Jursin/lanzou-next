import type { AppConfig } from '@/shared/types'

export const VERSION = '0.1.0'

export const LANZOU_URL = 'https://up.woozooo.com'

/** 默认 User-Agent */
export const DEFAULT_USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36'

/** User-Agent 预设 */
export const USER_AGENT_PRESETS = [
  {
    label: 'Chrome',
    value:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36',
  },
  {
    label: 'Edge',
    value:
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36 Edg/147.0.0.0',
  },
  {
    label: 'Safari',
    value:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.3 Safari/605.1.15',
  },
  {
    label: 'Firefox',
    value: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:150.0) Gecko/20100101 Firefox/150.0',
  },
] as const

/** 可选的日志级别（与后端 LevelFilter 对应） */
export const LOG_LEVELS = ['error', 'warn', 'info', 'debug', 'trace'] as const

export type LogLevel = (typeof LOG_LEVELS)[number]

export const DEFAULT_CONFIG: AppConfig = {
  lanzouUrl: LANZOU_URL,
  userAgent: DEFAULT_USER_AGENT,
  downloadDir: '',
  setDefaultDownloadDir: false,
  themeSource: 'auto',
  colorScheme: 'glacier',
  uploadMax: 1,
  downloadMax: 2,
  uploadWarningSize: 7,
  splitSize: 100,
  minimizeToTrayOnClose: true,
  lightweightMode: true,
  developerMode: false,
  logLevel: 'warn',
  autoCheckUpdate: true,
  betaUpdate: false,
}

/** 关于页外部链接 */
export const ABOUT_LINKS = [
  { label: 'GitHub', url: 'https://github.com/Jursin/lanzou-next', icon: 'LogoGithub' },
  { label: '更新日志', url: 'https://github.com/Jursin/lanzou-next/releases', icon: 'RocketOutline' },
  { label: '开源许可', url: 'https://github.com/Jursin/lanzou-next/blob/main/LICENSE', icon: 'DocumentTextOutline' },
  { label: '赞助与支持', url: 'https://afdian.com/a/jursin', icon: 'HeartOutline' },
] as const
