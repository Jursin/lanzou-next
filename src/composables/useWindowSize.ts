import {
  currentMonitor,
  getCurrentWindow,
  PhysicalPosition,
  PhysicalSize,
  primaryMonitor,
} from '@tauri-apps/api/window'

const DESIRED_SIZE = { width: 1260, height: 800 }

/* 修正分数缩放下窗口实际显示尺寸小于配置的问题，并在最终尺寸下居中 */
export async function calibrateWindowSize() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const win = getCurrentWindow()
    const { width: physicalWidth } = await win.innerSize()
    const cssWidth = window.innerWidth
    const ratio =
      physicalWidth > 0 && cssWidth > 0
        ? physicalWidth / cssWidth
        : window.devicePixelRatio || 1
    const target = { width: DESIRED_SIZE.width, height: DESIRED_SIZE.height }
    if (Number.isFinite(ratio) && ratio > 0 && Math.abs(ratio - 1) > 0.001) {
      target.width = Math.round(DESIRED_SIZE.width * ratio)
      target.height = Math.round(DESIRED_SIZE.height * ratio)
      await win.setSize(new PhysicalSize(target.width, target.height))
    }
    const monitor =
      (await currentMonitor()) || (await primaryMonitor())
    if (monitor) {
      const wa = monitor.workArea
      await win.setPosition(
        new PhysicalPosition(
          wa.position.x + Math.round((wa.size.width - target.width) / 2),
          wa.position.y + Math.round((wa.size.height - target.height) / 2),
        ),
      )
    }
    await win.show()
  } catch {
    /* 校准失败时按默认尺寸显示 */
  }
}