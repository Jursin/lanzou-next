import type { Component } from 'vue'
import {
  ArchiveOutline,
  BookOutline,
  CodeSlashOutline,
  DocumentOutline,
  DocumentTextOutline,
  FolderOpenOutline,
  ImageOutline,
  LogoAndroid,
  MusicalNotesOutline,
  TerminalOutline,
  VideocamOutline,
} from '@vicons/ionicons5'

/**
 * 按扩展名返回不同文件类型的图标组件。
 * 未知类型则使用通用文档图标兜底。
 */
export function getFileIconComponent(filename: string, isFolder = false): Component {
  if (isFolder) return FolderOpenOutline

  const ext = getExtension(filename).toLowerCase()
  if (!ext) return DocumentOutline

  if ('jpg,jpeg,png,webp,gif,svg,bmp,tiff,tif,icc,ico'.includes(ext)) return ImageOutline
  if ('mp4,avi,mkv,mov,flv,wmv,rm,rmvb,m4v'.includes(ext)) return VideocamOutline
  if ('mp3,wav,flac,aac,ogg,wma,mid,midi'.includes(ext)) return MusicalNotesOutline
  if ('zip,rar,7z,tar,gz,gzip,bz2,xz,lz4,zst,arj,deb,rpm,cab,bzip2,dmg,pkg,iso,img,gho'.includes(ext))
    return ArchiveOutline
  if ('epub,mobi,azw,azw3,fb2,cbz,cbr,djvu'.includes(ext)) return BookOutline
  if ('apk,apkm,apks,xapk'.includes(ext)) return LogoAndroid
  if ('doc,docx,rdoc,rdocx,pdf,ppt,pptx,xls,xlsx,txt'.includes(ext)) return DocumentTextOutline
  if (
    'lua,conf,cfg,ini,yaml,yml,json,toml,xml,html,css,scss,sass,less,php,py,js,ts,jsx,tsx,rb,go,rs,java,c,cpp,h,hpp,swift,kt,kts,sql,md,rst,tex,latex'.includes(
      ext,
    )
  )
    return CodeSlashOutline
  if ('bat,cmd,ps1,sh,exe,dll,bin,msu,so'.includes(ext)) return TerminalOutline

  return DocumentOutline
}

/**
 * 按扩展名返回 iconColor。
 * 使用 m3 design token，保证与主题适配。
 */
export function getFileIconInfo(filename: string): { iconColor: string } {
  const ext = getExtension(filename).toLowerCase()
  if (!ext) return { iconColor: 'var(--m3-on-surface-variant)' }

  // 图片
  if ('jpg,jpeg,png,webp,gif,svg,bmp,tiff,tif,icc,ico'.includes(ext)) return { iconColor: 'var(--m3-warning)' }
  // 视频
  if ('mp4,avi,mkv,mov,flv,wmv,rm,rmvb,m4v'.includes(ext)) return { iconColor: 'var(--m3-primary-container)' }
  // 音频
  if ('mp3,wav,flac,aac,ogg,wma,mid,midi'.includes(ext)) return { iconColor: '#e91e63' }
  // PDF
  if (ext === 'pdf') return { iconColor: 'var(--m3-error)' }
  // Office 文档
  if ('doc,docx,rdoc,rdocx'.includes(ext)) return { iconColor: 'color-mix(in srgb, var(--m3-primary) 78%, black)' }
  // 表格
  if ('xls,xlsx,csv,ods,fods,odt'.includes(ext)) return { iconColor: '#2e7d32' }
  // 演示
  if ('ppt,pptx,odp'.includes(ext)) return { iconColor: '#ef6c00' }
  // 磁盘镜像
  if ('iso,img,gho'.includes(ext)) return { iconColor: '#546e7a' }
  // 压缩包
  if ('zip,rar,7z,tar,gz,gzip,bz2,xz,lz4,zst,arj,deb,rpm,cab,bzip2,dmg,pkg'.includes(ext))
    return { iconColor: 'var(--m3-info)' }
  // 电子书
  if ('epub,mobi,azw,azw3,fb2,cbz,cbr,djvu'.includes(ext)) return { iconColor: 'var(--m3-tertiary-container)' }
  // CAD
  if ('dwg,dxf,cad,hwt'.includes(ext)) return { iconColor: '#0d47a1' }
  // 字体
  if ('ttf,otf,woff,woff2,ttc,eot,colr,cvt'.includes(ext)) return { iconColor: 'var(--m3-outline)' }
  // Apple 包
  if ('ipa,app,imazingapp,plist,xcassets,mobileconfig'.includes(ext)) return { iconColor: '#616161' }
  // 可执行 / 脚本
  if ('exe,dll,so,bin,msu,cmd,bat,ps1,sh'.includes(ext)) return { iconColor: 'var(--m3-on-surface-variant)' }
  // Android
  if ('apk,apkm,apks,xapk'.includes(ext)) return { iconColor: '#388e3c' }
  // Chrome 扩展
  if (ext === 'crx') return { iconColor: '#1e8e3e' }
  // Java
  if ('jar,jar_sign,jnlp'.includes(ext)) return { iconColor: '#ff6d00' }
  // 开发配置
  if (
    [
      'lua',
      'conf',
      'cfg',
      'ini',
      'yaml',
      'yml',
      'json',
      'toml',
      'xml',
      'html',
      'css',
      'scss',
      'sass',
      'less',
      'php',
      'py',
      'js',
      'ts',
      'jsx',
      'tsx',
      'rb',
      'go',
      'rs',
      'java',
      'c',
      'cpp',
      'h',
      'hpp',
      'swift',
      'kt',
      'kts',
      'sql',
      'md',
      'rst',
      'tex',
      'latex',
    ].includes(ext)
  )
    return { iconColor: '#78909c' }
  // 脑图 / 笔记
  if (ext === 'xmind') return { iconColor: '#ab47bc' }
  if (ext === 'jupyter') return { iconColor: '#f3712b' }
  // 数据库
  if (ext === 'db' || ext === 'xda') return { iconColor: 'var(--m3-outline-variant)' }
  // 游戏 / 特定格式
  if ('w3x,cpk,osz,osk,ct,ke,cetrainer,it,ssf,bds,bdi,enc,txf,lolgezi'.includes(ext))
    return { iconColor: 'var(--m3-primary)' }
  if ('appimage,rp,rplib,xpa,accdb,ce,e,z'.includes(ext))
    return { iconColor: ext === 'z' ? 'var(--m3-primary-container)' : 'var(--m3-primary)' }

  return { iconColor: 'var(--m3-on-surface-variant)' }
}

/** 从文件名提取扩展名（不含点） */
function getExtension(filename: string): string {
  const dot = filename.lastIndexOf('.')
  return dot === -1 ? '' : filename.slice(dot + 1)
}
