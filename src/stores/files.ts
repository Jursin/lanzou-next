import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import type { CrumbsInfo, LsFile } from '@/shared/types'
import { lanzouLs } from '@/shared/api'

export const useFilesStore = defineStore('files', () => {
  const folderId = ref<number>(-1)
  const files = ref<LsFile[]>([])
  const crumbs = ref<CrumbsInfo[]>([{ id: '-1', name: '根目录' }])
  const loading = ref(false)
  const error = ref('')

  const folders = computed(() => files.value.filter((f) => f.type === 'folder'))
  const fileItems = computed(() => files.value.filter((f) => f.type === 'file'))

  async function load(fid?: number) {
    loading.value = true
    error.value = ''
    try {
      const result = await lanzouLs(fid ?? folderId.value, true)
      files.value = result.files
      // 面包屑直接用 API 返回的完整路径（根目录由后端拼上）
      crumbs.value = result.info.length ? result.info : [{ id: String(fid ?? folderId.value), name: '根目录' }]
      if (fid !== undefined) folderId.value = fid
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  function refresh() {
    return load(folderId.value)
  }

  return { folderId, files, crumbs, loading, error, folders, fileItems, load, refresh }
})
