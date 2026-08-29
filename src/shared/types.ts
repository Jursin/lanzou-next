/** 蓝奏云文件/文件夹 */
export interface LsFile {
  name: string
  id: string
  type: 'file' | 'folder'
  icon?: string
  size?: string
  time?: string
  downs?: string
}

export interface CrumbsInfo {
  id: string
  name: string
}

export interface LsResult {
  info: CrumbsInfo[]
  files: LsFile[]
}

/** 分享链接类型 */
export enum ShareType {
  File = 'file',
  Folder = 'folder',
}

/** 分享解析结果 */
export interface ShareInfo {
  type: ShareType
  name: string
  pwd?: string
}

/** 下载任务 */
export interface DownloadTask {
  id: string
  url: string
  pwd?: string
  dir: string
  name?: string
}

/** 下载进度事件 */
export interface DownloadProgress {
  id: string
  name: string
  downloaded: number
  total: number
  speed: number
  filePath?: string
}

/** 上传任务 */
export interface UploadTask {
  id: string
  path: string
  folderId: number
  name?: string
  /** 超出大小限制的文件是否分片上传 */
  chunkOversized?: boolean
}

/** 上传预检超限文件 */
export interface OversizedFile {
  path: string
  name: string
  relPath: string
  size: number
}

/** 上传预检结果 */
export interface PrecheckResult {
  /** 账号单文件大小限制（字节），获取失败时为 null（视为不限制） */
  maxSize: number | null
  oversized: OversizedFile[]
}

/** 合并下载分片文件 */
export interface MergePart {
  id: string
  name: string
}

/** 合并下载任务 */
export interface MergeDownloadTask {
  id: string
  files: MergePart[]
  dir: string
  keepParts: boolean
}

/** 上传进度事件 */
export interface UploadProgress {
  id: string
  name: string
  uploaded: number
  total: number
  speed: number
}

/** 文件/文件夹操作通用结果 */
export interface OpResult {
  ok: boolean
  message: string
}

/** 文件描述 */
export interface FileDesc {
  name?: string
  desc?: string
}

/** 分享信息（详情） */
export interface ShareDetail {
  hasPwd: boolean
  pwd?: string
  url?: string
  name?: string
}

/** 分享文件夹中的文件 */
export interface ShareFile {
  name: string
  size: string
  time: string
  url: string
}

/** 分享文件夹解析结果 */
export interface ShareFolder {
  name: string
  size: string
  list: ShareFile[]
}

/** 回收站条目 */
export interface RecycleItem {
  id: string
  type: 'file' | 'folder'
  name: string
  size: string
  time: string
}

/** 回收站文件夹内的子文件（只读） */
export interface RecycleFile {
  name: string
  size: string
}

/** 待移动项 */
export interface MoveTarget {
  id: string
  name: string
  type: 'file' | 'folder'
}

export interface Cookie {
  name: string
  value: string
  domain?: string
  path?: string
  secure?: boolean
}

/** 账号信息（profile 解析结果） */
export interface Profile {
  isLogin: boolean
  /** 个性域名 */
  domain?: string
  /** 最近登录时间 */
  lastLogin?: string
  /** 允许上传类型 */
  supportList: string[]
  /** 单个文件大小限制 */
  maxSize?: string
  /** 安全验证（手机号） */
  verification?: string
}

export type ThemeSource = 'light' | 'dark' | 'auto'

/** 应用配置（与 Rust AppConfig 对应） */
export interface AppConfig {
  lanzouUrl?: string
  domain?: string
  userAgent?: string
  cookies?: Cookie[]
  downloadDir?: string
  setDefaultDownloadDir?: boolean
  themeSource?: ThemeSource
  colorScheme?: string
  uploadMax?: number
  downloadMax?: number
  /** 上传流量警戒线（G），设置了值即开启警戒，清空即关闭 */
  uploadWarningSize?: number
  /** 文件分片大小（MB），分片上传时单片大小 */
  splitSize?: number
  minimizeToTrayOnClose?: boolean
  lightweightMode?: boolean
  devTools?: boolean
  /** 日志级别: error | warn | info | debug | trace */
  logLevel?: string
  /** 启动时自动检查更新 */
  autoCheckUpdate?: boolean
  /** 接收测试版更新 */
  betaUpdate?: boolean
  /** 上次检查更新时间（Unix 毫秒时间戳） */
  lastCheckUpdateTime?: number
  /** GitHub 加速地址，留空则直连 */
  githubProxyUrl?: string
  /** 上传记录上限（0 = 无限制） */
  uploadHistoryLimit?: number
  /** 下载记录上限（0 = 无限制） */
  downloadHistoryLimit?: number
  /** 解析记录上限（0 = 无限制） */
  parseHistoryLimit?: number
}

/** 更新检查结果 */
export interface UpdateInfo {
  version: string
  name: string
  url: string
  publishedAt?: string
  isPrerelease: boolean
  /** 当前平台安装包下载地址 */
  assetUrl?: string
  /** 安装包文件名 */
  assetName?: string
}

/** 更新下载进度 */
export interface UpdateDownloadProgress {
  downloaded: number
  total: number
}
