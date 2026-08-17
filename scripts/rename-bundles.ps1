# 构建后把 NSIS 安装包按 mainBinaryName 改名（安装包文件名英文，显示名仍为中文 productName）
$dir = Join-Path $PSScriptRoot '..\src-tauri\target\release\bundle\nsis'
if (!(Test-Path $dir)) { exit 0 }

Get-ChildItem $dir | Where-Object {
  $_.Name -match '^[^_]+_(\d+\.\d+\.\d+)_([a-z0-9]+)-setup\.exe$'
} | ForEach-Object {
  $newName = "lanzou-next_$($Matches[1])_$($Matches[2])-setup.exe"
  $newPath = Join-Path $dir $newName
  if ($_.Name -ne $newName) {
    # 覆盖旧残留（多次构建同名冲突时）
    if (Test-Path $newPath) { Remove-Item $newPath -Force }
    Rename-Item -Path $_.FullName -NewName $newName -Force
    Write-Output "renamed -> $newName"
  } else {
    Write-Output "skip (already named): $newName"
  }
}
