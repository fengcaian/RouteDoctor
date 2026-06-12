# 一键下载 Npcap SDK 到项目本地（仅 Windows 开发者需要）
#
# 用途：pcap crate 链接 wpcap.lib 时需要 Npcap SDK 中的 .lib 文件。
# 本脚本下载官方 SDK 并解压到 src-tauri/.npcap-sdk/，build.rs 会自动检测此路径。
#
# 用法：在仓库根目录或 src-tauri 下执行 powershell ./scripts/setup-npcap-sdk.ps1
#
# 不会被 git 追踪（已在 .gitignore），每个开发者跑一次即可。

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Npcap SDK 版本：可以在 https://npcap.com/dist/ 找到最新版本
$SdkVersion = '1.13'
$Url = "https://npcap.com/dist/npcap-sdk-$SdkVersion.zip"

# 解压目标：脚本所在的 src-tauri/.npcap-sdk
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SrcTauriDir = Split-Path -Parent $ScriptDir
$SdkDir = Join-Path $SrcTauriDir '.npcap-sdk'
$Marker = Join-Path $SdkDir 'Lib\x64\wpcap.lib'

if (Test-Path $Marker) {
    Write-Host "[setup-npcap-sdk] SDK 已存在: $SdkDir" -ForegroundColor Green
    Write-Host "[setup-npcap-sdk] 跳过下载。如需重新下载请先删除该目录。"
    exit 0
}

$Tmp = Join-Path $env:TEMP "npcap-sdk-$SdkVersion.zip"
Write-Host "[setup-npcap-sdk] 正在从 $Url 下载 ..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $Url -OutFile $Tmp

$Size = (Get-Item $Tmp).Length
Write-Host "[setup-npcap-sdk] 下载完成 ($([Math]::Round($Size / 1024, 1)) KB)。"

if (Test-Path $SdkDir) { Remove-Item $SdkDir -Recurse -Force }
New-Item -ItemType Directory -Path $SdkDir -Force | Out-Null
Expand-Archive -Path $Tmp -DestinationPath $SdkDir -Force
Remove-Item $Tmp -Force

if (Test-Path $Marker) {
    Write-Host "[setup-npcap-sdk] 已就绪: $SdkDir" -ForegroundColor Green
    Write-Host "[setup-npcap-sdk] cargo build 现在应该能成功链接 pcap crate。"
} else {
    Write-Host "[setup-npcap-sdk] ERROR: 解压后未找到 wpcap.lib，请检查 SDK 包内容。" -ForegroundColor Red
    exit 1
}
