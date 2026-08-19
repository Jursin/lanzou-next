import { ref } from 'vue'
import { defineStore } from 'pinia'
import { listen } from '@tauri-apps/api/event'

import type { DownloadProgress, UploadProgress } from '@/shared/types'
import {
  configGet,
  lanzouCancelTransfer,
  lanzouDownload,
  lanzouDownloadById,
  lanzouMergeDownload,
  lanzouRmFolder,
  lanzouUpload,
} from '@/shared/api'
import { useUploadTrafficStore } from '@/stores/uploadTraffic'

export type TransferItem = {
  id: string
  name: string
  kind: 'upload' | 'download'
  status: 'pending' | 'running' | 'done' | 'error' | 'paused'
  uploaded: number
  total: number
  speed: number
  error?: string
  /** 上传: 文件路径; 下载: file_id */
  payload?: string
  /** 分享链接下载：直填分享 URL（解析页/同步页），优先于 payload */
  url?: string
  /** 分享提取码 */
  pwd?: string
  /** 上传目标文件夹 id */
  folderId?: number
  isFolder?: boolean
  /** 开始时间戳 */
  startedAt?: number
  /** 结束时间戳 */
  finishedAt?: number
  /** 暂停前累计已花时间(ms)，恢复后在此基础上继续累计 */
  elapsedMs?: number
  /** 下载保存路径 */
  filePath?: string
  /** 下载子目录（相对下载目录，如"文件夹A/子文件夹B"） */
  dir?: string
  /** 本地文件是否已丢失（下载完成后文件被删除） */
  lost?: boolean
  /** 是否被用户手动暂停 */
  userPaused?: boolean
  /** 超出大小限制的文件是否分片上传 */
  chunkOversized?: boolean
  /** 分片上传创建的云端子文件夹 id（用于删除未完成任务时清理云端文件夹） */
  chunkFolderIds?: number[]
  /** 合并下载：分片文件列表（存在时按合并下载处理） */
  mergeFiles?: { id: string; name: string }[]
  /** 合并下载：合并后是否保留分片文件 */
  keepParts?: boolean
}

const MAX_CONCURRENT = 2
const STORAGE_KEY = 'lanzou.transfers'

export const useTransferStore = defineStore('transfer', () => {
  const trafficStore = useUploadTrafficStore()
  const uploads = ref<TransferItem[]>([])
  const downloads = ref<TransferItem[]>([])
  const completed = ref<TransferItem[]>([])
  const runningUploads = ref(0)
  const runningDownloads = ref(0)
  const downloadDir = ref('')
  let listenerReady = false

  function persist() {
    try {
      const data = {
        uploads: uploads.value.map((i) => ({ ...i, status: i.status === 'running' ? 'pending' : i.status, speed: 0 })),
        downloads: downloads.value.map((i) => ({
          ...i,
          status: i.status === 'running' ? 'pending' : i.status,
          speed: 0,
        })),
        completed: completed.value,
      }
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data))
    } catch {
      /* ignore */
    }
  }

  function restore() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      if (!raw) return
      const data = JSON.parse(raw)
      if (Array.isArray(data.uploads)) uploads.value = data.uploads
      if (Array.isArray(data.downloads)) downloads.value = data.downloads
      if (Array.isArray(data.completed)) completed.value = data.completed
    } catch {
      /* ignore */
    }
  }

  async function init() {
    restore()
    try {
      const cfg = await configGet()
      downloadDir.value = cfg.downloadDir || ''
    } catch {
      /* ignore */
    }
  }

  function findIn(list: TransferItem[], id: string) {
    return list.find((i) => i.id === id)
  }

  function addUpload(item: TransferItem) {
    uploads.value.unshift(item)
    persist()
  }

  function addDownload(item: TransferItem) {
    downloads.value.unshift(item)
    persist()
  }

  /** 启动上传任务（受并发限制） */
  async function startUploadTask(item: TransferItem) {
    if (item.status === 'running') return
    if (runningUploads.value >= MAX_CONCURRENT) {
      item.status = 'pending'
      return
    }
    item.status = 'running'
    item.startedAt = Date.now()
    item.finishedAt = undefined
    item.userPaused = false
    runningUploads.value++
    try {
      await lanzouUpload({
        id: item.id,
        path: item.payload || '',
        folderId: item.folderId ?? -1,
        name: item.name,
        chunkOversized: item.chunkOversized,
      })
    } catch (e) {
      // 用户暂停导致的取消不视为错误
      if (!item.userPaused) {
        item.status = 'error'
        item.error = e instanceof Error ? e.message : String(e)
        item.finishedAt = Date.now()
      }
    } finally {
      runningUploads.value--
      const stillListed = findIn(uploads.value, item.id)
      if (stillListed && !item.userPaused && (stillListed.status as string) === 'running') {
        item.status = 'done'
        item.finishedAt = Date.now()
        moveToCompleted(uploads.value, item)
      }
      dispatchNext('upload')
      persist()
    }
  }

  /** 启动下载任务（受并发限制） */
  async function startDownloadTask(item: TransferItem) {
    if (item.status === 'running') return
    if (runningDownloads.value >= MAX_CONCURRENT) {
      item.status = 'pending'
      return
    }
    item.status = 'running'
    item.startedAt = Date.now()
    item.finishedAt = undefined
    item.userPaused = false
    runningDownloads.value++
    try {
      const dir = item.dir ? `${downloadDir.value}/${item.dir}` : downloadDir.value
      if (item.mergeFiles?.length) {
        // 合并下载（分片文件）
        await lanzouMergeDownload({
          id: item.id,
          files: item.mergeFiles,
          dir: dir || '',
          keepParts: !!item.keepParts,
        })
        item.filePath = dir ? `${dir}/${item.name}` : undefined
      } else if (item.url) {
        // 分享链接直下（解析/同步页添加的任务）
        await lanzouDownload({ id: item.id, url: item.url, pwd: item.pwd, dir: dir || '', name: item.name })
      } else {
        await lanzouDownloadById(item.id, item.payload || item.id, !!item.isFolder, dir || undefined, item.name)
      }
      item.filePath = dir ? `${dir}/${item.name}` : undefined
    } catch (e) {
      if (!item.userPaused) {
        item.status = 'error'
        item.error = e instanceof Error ? e.message : String(e)
        item.finishedAt = Date.now()
      }
    } finally {
      runningDownloads.value--
      const stillListed = findIn(downloads.value, item.id)
      if (stillListed && !item.userPaused && (stillListed.status as string) === 'running') {
        item.status = 'done'
        item.finishedAt = Date.now()
        moveToCompleted(downloads.value, item)
      }
      dispatchNext('download')
      persist()
    }
  }

  /** 空闲槽位时启动下一个等待任务 */
  function dispatchNext(kind: 'upload' | 'download') {
    const list = kind === 'upload' ? uploads.value : downloads.value
    const running = kind === 'upload' ? runningUploads.value : runningDownloads.value
    if (running >= MAX_CONCURRENT) return
    // 仅自动启动初始 pending 任务；error/paused 需用户手动点击开始
    const next = list.find((i) => i.status === 'pending')
    if (!next) return
    if (kind === 'upload') void startUploadTask(next)
    else void startDownloadTask(next)
  }

  /** 开始单个任务 */
  function startItem(kind: 'upload' | 'download', id: string) {
    const list = kind === 'upload' ? uploads.value : downloads.value
    const item = findIn(list, id)
    if (!item) return
    item.status = 'pending'
    item.userPaused = false
    item.error = undefined
    if (kind === 'upload') void startUploadTask(item)
    else void startDownloadTask(item)
    persist()
  }

  /** 批量加入队列：将列表中 pending 状态的任务按并发依次启动 */
  function startAll(kind: 'upload' | 'download') {
    dispatchNext(kind)
  }

  /** 暂停单个任务 */
  function pauseItem(kind: 'upload' | 'download', id: string) {
    const list = kind === 'upload' ? uploads.value : downloads.value
    const item = findIn(list, id)
    if (!item) return
    if (item.status === 'running' || item.status === 'pending') {
      item.userPaused = true
      if (item.status === 'running') {
        // 取消后端请求；上传恢复时重新上传，下载保留 .download 临时文件续传
        lanzouCancelTransfer(id).catch(() => {})
      }
      item.status = 'paused'
      item.finishedAt = Date.now()
      // 累计本次已花时间，恢复后续上
      item.elapsedMs = (item.elapsedMs || 0) + Math.max(0, Date.now() - (item.startedAt || Date.now()))
      item.startedAt = undefined
      persist()
    }
  }

  /** 重试（重新下载/上传）单个任务 */
  function retryItem(kind: 'upload' | 'download', id: string) {
    // 从已完成列表找
    const completedIdx = completed.value.findIndex((i) => i.id === id)
    if (completedIdx >= 0) {
      const item = completed.value[completedIdx]
      completed.value.splice(completedIdx, 1)
      const reset = {
        ...item,
        status: 'pending' as const,
        uploaded: 0,
        total: 0,
        speed: 0,
        startedAt: undefined,
        finishedAt: undefined,
        elapsedMs: 0,
        error: undefined,
        lost: false,
        userPaused: false,
      }
      if (kind === 'upload') {
        uploads.value.unshift(reset)
        startAll('upload')
      } else {
        downloads.value.unshift(reset)
        startAll('download')
      }
      persist()
      return
    }
    startItem(kind, id)
  }

  /** 从已完成列表移除 */
  function removeCompleted(id: string) {
    const idx = completed.value.findIndex((i) => i.id === id)
    if (idx >= 0) completed.value.splice(idx, 1)
    persist()
  }

  function removeItem(kind: 'upload' | 'download', id: string) {
    const list = kind === 'upload' ? uploads.value : downloads.value
    const item = findIn(list, id)
    if (item && item.status === 'running') {
      item.userPaused = true
      lanzouCancelTransfer(id).catch(() => {})
    }
    // 删除未完成的分片上传任务时，清理云端创建的分片子文件夹（避免残留空/半成品文件夹）
    if (kind === 'upload' && item?.chunkFolderIds?.length) {
      for (const fid of item.chunkFolderIds) {
        lanzouRmFolder(String(fid)).catch(() => {})
      }
    }
    const idx = list.findIndex((i) => i.id === id)
    if (idx >= 0) list.splice(idx, 1)
    persist()
  }

  /** 标记已完成文件是否丢失 */
  function markLost(id: string, lost: boolean) {
    const item = findIn(completed.value, id)
    if (item) item.lost = lost
  }

  function moveToCompleted(list: TransferItem[], item: TransferItem) {
    const idx = list.findIndex((i) => i.id === item.id)
    if (idx >= 0) list.splice(idx, 1)
    completed.value.unshift({ ...item, status: 'done', speed: 0 })
    persist()
  }

  async function setupListeners() {
    if (listenerReady) return
    // 分片上传创建的云端子文件夹：记录到任务上，删除任务时用于清理云端文件夹
    await listen<{ taskId: string; folderId: number }>('upload:chunk-folder', (e) => {
      const item = findIn(uploads.value, e.payload.taskId)
      if (!item) return
      if (!item.chunkFolderIds) item.chunkFolderIds = []
      if (!item.chunkFolderIds.includes(e.payload.folderId)) item.chunkFolderIds.push(e.payload.folderId)
    })
    await listen<DownloadProgress>('download:progress', (e) => {
      const p = e.payload
      const item = findIn(downloads.value, p.id)
      if (!item) return
      if (item.mergeFiles) {
        // 合并下载：进度事件仅更新展示；收到带 file_path 的最终事件才标记完成
        // （避免 invoke 返回时最后一条事件未到达导致状态停留在 (i/n)）
        item.name = p.name
        item.uploaded = p.downloaded
        if (p.total > item.total) item.total = p.total
        item.speed = p.speed
        if (p.filePath) {
          item.filePath = p.filePath
          item.status = 'done'
          item.finishedAt = Date.now()
          moveToCompleted(downloads.value, item)
        } else {
          item.status = 'running'
        }
        return
      }
      if (p.total > 0 && p.downloaded >= p.total) {
        item.uploaded = p.downloaded
        if (p.total > item.total) item.total = p.total
        item.speed = p.speed
        if (p.filePath) item.filePath = p.filePath
        item.status = 'done'
        item.finishedAt = Date.now()
        moveToCompleted(downloads.value, item)
      } else if (!item.userPaused) {
        item.name = p.name
        item.uploaded = p.downloaded
        if (p.total > item.total) item.total = p.total
        item.speed = p.speed
        if (p.filePath) item.filePath = p.filePath
        item.status = 'running'
      }
    })
    await listen<UploadProgress>('upload:progress', (e) => {
      const p = e.payload
      const item = findIn(uploads.value, p.id)
      if (!item) return
      // 每日上传流量统计：以任务 id + 本次启动时间作为尝试唯一标识，重试会累加；完成时强制持久化
      const done = p.uploaded >= p.total && p.total > 0
      trafficStore.setRecord(`${p.id}@${item.startedAt || 0}`, p.uploaded, done)
      if (done) {
        item.uploaded = p.uploaded
        item.total = p.total
        item.status = 'done'
        item.finishedAt = Date.now()
        moveToCompleted(uploads.value, item)
      } else if (!item.userPaused) {
        item.name = p.name
        item.uploaded = p.uploaded
        item.total = p.total
        item.speed = p.speed
        item.status = 'running'
      }
    })
    listenerReady = true
  }

  return {
    uploads,
    downloads,
    completed,
    runningUploads,
    runningDownloads,
    downloadDir,
    init,
    addUpload,
    addDownload,
    startItem,
    startAll,
    pauseItem,
    retryItem,
    removeItem,
    removeCompleted,
    markLost,
    setupListeners,
  }
})
