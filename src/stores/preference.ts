import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import type { AppConfig } from '@/shared/types'
import { DEFAULT_CONFIG } from '@/shared/constants'
import { configGet, configSet } from '@/shared/api'

export const usePreferenceStore = defineStore('preference', () => {
  const config = ref<AppConfig>({ ...DEFAULT_CONFIG })

  const downloadDir = computed(() => config.value.downloadDir)

  async function load() {
    const cfg = await configGet()
    config.value = { ...DEFAULT_CONFIG, ...cfg }
  }

  async function update(patch: Partial<AppConfig>) {
    config.value = { ...config.value, ...patch }
    try {
      await configSet(patch)
    } catch (e) {
      console.warn('save config failed', e)
    }
  }

  return { config, downloadDir, load, update }
})
