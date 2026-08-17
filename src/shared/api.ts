import { invoke } from '@tauri-apps/api/core'

import type {
  AppConfig,
  DownloadTask,
  FileDesc,
  LsResult,
  MergeDownloadTask,
  MoveTarget,
  OpResult,
  PrecheckResult,
  Profile,
  RecycleFile,
  RecycleItem,
  ShareDetail,
  ShareFolder,
  ShareInfo,
  UploadTask,
  UpdateInfo,
} from '@/shared/types'

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args)
}

// config
export const configGet = () => call<AppConfig>('config_get')
export const configSet = (cfg: AppConfig) => call<void>('config_set', { cfg })
export const configReset = () => call<AppConfig>('config_reset')
export const configClear = (keys: string[]) => call<void>('config_clear', { keys })

// 日志
export const logGetFile = () => call<string>('log_get_file')
export const logClear = () => call<void>('log_clear')

// 文件列表
export const lanzouLs = (folderId?: number, folderFirst?: boolean) =>
  call<LsResult>('lanzou_ls', { folderId, folderFirst })

export const lanzouProfile = () => call<Profile>('lanzou_profile')

// 分享 / 下载
export const lanzouShareInfo = (url: string, pwd?: string) => call<ShareInfo>('lanzou_share_info', { url, pwd })

export const lanzouShareFolder = (url: string, pwd?: string) => call<ShareFolder>('lanzou_share_folder', { url, pwd })

export const lanzouDownload = (task: DownloadTask) => call<void>('lanzou_download', { task })

export const lanzouDownloadById = (taskId: string, id: string, isFolder: boolean, dir?: string, name?: string) =>
  call<void>('lanzou_download_by_id', { taskId, id, isFolder, dir, name })

export const lanzouCancelTransfer = (taskId: string) => call<void>('lanzou_cancel_transfer', { taskId })

export const lanzouUpload = (task: UploadTask) => call<void>('lanzou_upload', { task })

export const lanzouUploadPrecheck = (path: string) => call<PrecheckResult>('lanzou_upload_precheck', { path })

export const lanzouMergeDownload = (task: MergeDownloadTask) => call<void>('lanzou_merge_download', { task })

export const lanzouCheckPath = (path: string) => call<boolean>('lanzou_check_path', { path })

export const lanzouDeleteLocal = (path: string, dir?: string, name?: string) =>
  call<void>('lanzou_delete_local', { path, dir, name })

export const lanzouDeleteLocalDir = (dir: string, name: string) => call<void>('lanzou_delete_local_dir', { dir, name })

// 文件 / 文件夹操作
export const lanzouMkdir = (parentId: number, name: string, description?: string) =>
  call<string>('lanzou_mkdir', { parentId, name, description })

export const lanzouRmFile = (fileId: string) => call<OpResult>('lanzou_rm_file', { fileId })
export const lanzouRmFolder = (folderId: string) => call<OpResult>('lanzou_rm_folder', { folderId })

export const lanzouRenameFile = (fileId: string, name: string) => call<OpResult>('lanzou_rename_file', { fileId, name })

export const lanzouRenameFolder = (folderId: string, name: string, description?: string) =>
  call<OpResult>('lanzou_rename_folder', { folderId, name, description })

export const lanzouMove = (items: MoveTarget[], targetId: number) => call<OpResult>('lanzou_move', { items, targetId })

export const lanzouRecycleList = () => call<RecycleItem[]>('lanzou_recycle_list')

export const lanzouRecycleFiles = (folderId: string) => call<RecycleFile[]>('lanzou_recycle_files', { folderId })

export const lanzouRecycleAction = (id: string, fileType: 'file' | 'folder', action: 'restore' | 'delete') =>
  call<OpResult>('lanzou_recycle_action', { id, fileType, action })

export const lanzouSetFileAccess = (fileId: string, shows: number, shownames: string) =>
  call<OpResult>('lanzou_set_file_access', { fileId, shows, shownames })

export const lanzouSetFolderAccess = (folderId: string, shows: number, shownames: string) =>
  call<OpResult>('lanzou_set_folder_access', { folderId, shows, shownames })

export const lanzouFileDescription = (fileId: string) => call<FileDesc>('lanzou_file_description', { fileId })

export const lanzouSetFileDescription = (fileId: string, desc: string) =>
  call<OpResult>('lanzou_set_file_description', { fileId, desc })

export const lanzouFileDetail = (fileId: string) => call<ShareDetail>('lanzou_file_detail', { fileId })

export const lanzouFolderDetail = (folderId: string) => call<ShareDetail>('lanzou_folder_detail', { folderId })

// 登录
export const lanzouLogin = (username: string, password: string) => call<Profile>('lanzou_login', { username, password })
export const lanzouLogout = () => call<void>('lanzou_logout')

// 更新
export const checkForUpdate = (beta?: boolean) => call<UpdateInfo | null>('check_for_update', { beta })
export const cancelDownload = () => call<void>('cancel_download')
export const downloadAndInstall = (info: UpdateInfo) => call<void>('download_and_install', { info })
