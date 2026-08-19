import { ref } from 'vue'
import { useMessage } from 'naive-ui'

import { lanzouRmFile, lanzouRmFolder, lanzouRecycleAction } from '@/shared/api'
import type { LsFile, RecycleItem } from '@/shared/types'

const recycleDeleting = ref(false)
const recycleDeletingItems = ref<RecycleItem[]>([])

const fileDeleting = ref(false)
const fileDeletingItems = ref<LsFile[]>([])

let recycleOnFinish: (() => void) | null = null
let fileOnFinish: (() => void) | null = null

export function setRecycleDeleteFinish(fn: (() => void) | null) {
  recycleOnFinish = fn
}

export function setFileDeleteFinish(fn: (() => void) | null) {
  fileOnFinish = fn
}

export function useRecycleDelete() {
  const message = useMessage()

  function startRecycleDelete(items: RecycleItem[]) {
    if (!items.length || recycleDeleting.value) return
    recycleDeleting.value = true
    recycleDeletingItems.value = items.slice()
    void runRecycleDelete()
  }

  async function runRecycleDelete() {
    try {
      for (const it of recycleDeletingItems.value) {
        await lanzouRecycleAction(it.id, it.type, 'delete')
      }
      message.success(`已彻底删除 ${recycleDeletingItems.value.length} 项`)
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
    } finally {
      recycleDeleting.value = false
      recycleDeletingItems.value = []
      if (recycleOnFinish) recycleOnFinish()
    }
  }

  function startFileDelete(items: LsFile[]) {
    if (!items.length || fileDeleting.value) return
    fileDeleting.value = true
    fileDeletingItems.value = items.slice()
    void runFileDelete()
  }

  async function runFileDelete() {
    try {
      for (const f of fileDeletingItems.value) {
        if (f.type === 'folder') await lanzouRmFolder(f.id)
        else await lanzouRmFile(f.id)
      }
      message.success(`已删除 ${fileDeletingItems.value.length} 项`)
    } catch (e) {
      message.error(e instanceof Error ? e.message : String(e))
    } finally {
      fileDeleting.value = false
      fileDeletingItems.value = []
      if (fileOnFinish) fileOnFinish()
    }
  }

  return { recycleDeleting, fileDeleting, startRecycleDelete, startFileDelete }
}