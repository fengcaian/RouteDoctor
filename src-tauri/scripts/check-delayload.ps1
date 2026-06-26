# 校验 routedoctor.exe 是否把 wpcap.dll 放到了延迟导入表（Delay Import Directory）
# 用法： powershell -ExecutionPolicy Bypass -File scripts/check-delayload.ps1 [exePath]
#
# 原理：直接读 PE 头，检查 IMAGE_DIRECTORY_ENTRY_IMPORT (#1) 与
#       IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT (#13) 两个目录里出现的 DLL 名。
# 不依赖 dumpbin，纯 PowerShell + .NET 即可。

param(
    [string]$ExePath = "$PSScriptRoot/../target/release/routedoctor.exe"
)

if (-not (Test-Path $ExePath)) {
    Write-Error "找不到 EXE: $ExePath"
    exit 1
}

$bytes = [System.IO.File]::ReadAllBytes($ExePath)

function Read-UInt16([byte[]]$b, [int]$o) { [BitConverter]::ToUInt16($b, $o) }
function Read-UInt32([byte[]]$b, [int]$o) { [BitConverter]::ToUInt32($b, $o) }

# DOS 头 -> e_lfanew
$peOffset = Read-UInt32 $bytes 0x3C
if ([System.Text.Encoding]::ASCII.GetString($bytes, $peOffset, 4) -ne "PE`0`0") {
    Write-Error "不是有效 PE 文件"
    exit 1
}

# COFF 头 4 字节 PE 签名后：Machine(2) NumberOfSections(2) ...
$coff = $peOffset + 4
$numSections = Read-UInt16 $bytes ($coff + 2)
$sizeOfOptionalHeader = Read-UInt16 $bytes ($coff + 16)
$optHdr = $coff + 20
$magic = Read-UInt16 $bytes $optHdr
$is64 = $magic -eq 0x20B
# 数据目录在 OptionalHeader 里的偏移：PE32 = 96，PE32+ = 112
$dataDirOffset = if ($is64) { $optHdr + 112 } else { $optHdr + 96 }

function Get-DataDir([int]$index) {
    $base = $dataDirOffset + ($index * 8)
    [pscustomobject]@{
        VirtualAddress = Read-UInt32 $bytes $base
        Size           = Read-UInt32 $bytes ($base + 4)
    }
}

$importDir       = Get-DataDir 1   # IMAGE_DIRECTORY_ENTRY_IMPORT
$delayImportDir  = Get-DataDir 13  # IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT

# 读 Section Header，用于把 RVA 转换成文件偏移
$sectionsOffset = $optHdr + $sizeOfOptionalHeader
$sections = for ($i = 0; $i -lt $numSections; $i++) {
    $s = $sectionsOffset + ($i * 40)
    [pscustomobject]@{
        VirtualAddress = Read-UInt32 $bytes ($s + 12)
        VirtualSize    = Read-UInt32 $bytes ($s + 8)
        RawDataOffset  = Read-UInt32 $bytes ($s + 20)
        RawDataSize    = Read-UInt32 $bytes ($s + 16)
    }
}

function Rva-To-Offset([uint32]$rva) {
    foreach ($s in $sections) {
        if ($rva -ge $s.VirtualAddress -and $rva -lt ($s.VirtualAddress + [Math]::Max($s.VirtualSize, $s.RawDataSize))) {
            return [int]($s.RawDataOffset + ($rva - $s.VirtualAddress))
        }
    }
    return -1
}

function Read-Asciiz([int]$offset) {
    $sb = New-Object System.Text.StringBuilder
    while ($offset -lt $bytes.Length -and $bytes[$offset] -ne 0) {
        [void]$sb.Append([char]$bytes[$offset])
        $offset++
    }
    $sb.ToString()
}

function List-Imports([uint32]$dirRva, [uint32]$dirSize, [int]$nameFieldOffset, [int]$entrySize) {
    if ($dirRva -eq 0) { return @() }
    $base = Rva-To-Offset $dirRva
    if ($base -lt 0) { return @() }
    $names = @()
    $cursor = $base
    while ($cursor -lt $bytes.Length) {
        $nameRva = Read-UInt32 $bytes ($cursor + $nameFieldOffset)
        if ($nameRva -eq 0) { break }
        $nameOff = Rva-To-Offset $nameRva
        if ($nameOff -lt 0) { break }
        $names += Read-Asciiz $nameOff
        $cursor += $entrySize
    }
    $names
}

# IMAGE_IMPORT_DESCRIPTOR：20 字节，Name 在偏移 12
$normalImports = List-Imports $importDir.VirtualAddress $importDir.Size 12 20

# IMAGE_DELAYLOAD_DESCRIPTOR：32 字节，DllNameRVA 在偏移 4
$delayImports  = List-Imports $delayImportDir.VirtualAddress $delayImportDir.Size 4 32

Write-Host "==== 普通导入 (IMAGE_DIRECTORY_ENTRY_IMPORT) ====" -ForegroundColor Cyan
$normalImports | ForEach-Object { Write-Host "  $_" }

Write-Host ""
Write-Host "==== 延迟导入 (IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT) ====" -ForegroundColor Cyan
if ($delayImports.Count -eq 0) { Write-Host "  (空)" } else { $delayImports | ForEach-Object { Write-Host "  $_" } }

Write-Host ""
$wpcapNormal = $normalImports | Where-Object { $_ -match '^wpcap\.dll$' }
$wpcapDelay  = $delayImports  | Where-Object { $_ -match '^wpcap\.dll$' }

if ($wpcapDelay -and -not $wpcapNormal) {
    Write-Host "[OK] wpcap.dll 已正确放入延迟导入表，没有装 Npcap 的机器也能启动 EXE" -ForegroundColor Green
    exit 0
} elseif ($wpcapNormal) {
    Write-Host "[FAIL] wpcap.dll 仍在普通导入表里，启动时会触发系统错误" -ForegroundColor Red
    exit 2
} else {
    Write-Host "[?] EXE 中没有 wpcap.dll 的导入（pcap 可能没被实际链接进来）" -ForegroundColor Yellow
    exit 0
}
