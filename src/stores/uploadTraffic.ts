import { ref } from 'vue'
import { defineStore } from 'pinia'

interface DayTrafficRecord {
  /** 日期，格式 YYYYMMDD */
  date: string
  /** 单个上传任务累计已传字节，key 为任务尝试唯一 id */
  record: Record<string, number>
}

const STORAGE_KEY = 'lanzou.uploadTraffic'

/** 每日上传流量统计（本地持久化，用于上传流量警戒） */
export const useUploadTrafficStore = defineStore('uploadTraffic', () => {
  const days = ref<DayTrafficRecord[]>([])
  let lastPersist = 0

  function dateStr(d = new Date()): string {
    return `${d.getFullYear()}${d.getMonth() + 1}${d.getDate()}`
  }

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(days.value))
    } catch {
      /* ignore */
    }
  }

  function restore() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY)
      if (!raw) return
      const data = JSON.parse(raw)
      if (Array.isArray(data)) days.value = data
    } catch {
      /* ignore */
    }
  }

  /** 今日累计已上传流量（字节） */
  function todaySize(): number {
    const day = days.value.find((d) => d.date === dateStr())
    if (!day) return 0
    return Object.values(day.record).reduce((sum, n) => sum + (Number(n) || 0), 0)
  }

  /** 记录某个上传任务当前已传字节（每个尝试一条记录，覆盖为最新进度；默认 1s 节流持久化） */
  function setRecord(uid: string, size: number, force = false) {
    const date = dateStr()
    let day = days.value.find((d) => d.date === date)
    if (!day) {
      day = { date, record: {} }
      days.value.push(day)
    }
    day.record[uid] = size
    const now = Date.now()
    if (force || now - lastPersist >= 1000) {
      lastPersist = now
      persist()
    }
  }

  return { days, todaySize, setRecord, restore }
})
