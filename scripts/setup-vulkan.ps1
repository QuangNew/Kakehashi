# Prepare build-time tools locally; no SDK installer, registry or driver changes.
$ErrorActionPreference = 'Stop'
$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskPackages = Join-Path $taskRoot '.native-tools/vulkan-packages'
$taskDestination = Join-Path $taskRoot '.native-tools/vulkan'
if ((Test-Path -LiteralPath (Join-Path $taskDestination 'Bin/glslc.exe')) -and
    (Test-Path -LiteralPath (Join-Path $taskDestination 'Include/vulkan/vulkan.hpp')) -and
    (Test-Path -LiteralPath (Join-Path $taskDestination 'Lib/vulkan-1.lib'))) { return }
New-Item -ItemType Directory -Force -Path $taskPackages | Out-Null
$taskDownloads = @(
    @{ File='7zr.exe'; Url='https://www.7-zip.org/a/7zr.exe'; Sha='256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282' },
    @{ File='vulkan-sdk-1.4.363.0.exe'; Url='https://sdk.lunarg.com/sdk/download/1.4.363.0/windows/vulkan_sdk.exe?Human=true'; Sha='94a82d378f7a5e3e54c9db7d2fb7016af136e14ac0a18dbf0f2f67a36352d141' }
)
foreach ($taskDownload in $taskDownloads) {
    $taskPath = Join-Path $taskPackages $taskDownload.File
    if (-not (Test-Path -LiteralPath $taskPath)) {
        Invoke-WebRequest -Uri $taskDownload.Url -UserAgent 'Mozilla/5.0' -OutFile $taskPath
    }
    if ((Get-FileHash -LiteralPath $taskPath -Algorithm SHA256).Hash -ne $taskDownload.Sha) {
        throw ('Build-tool checksum mismatch: ' + $taskPath)
    }
}
& python (Join-Path $PSScriptRoot 'unpack-vulkan.py') (Join-Path $taskPackages 'vulkan-sdk-1.4.363.0.exe') (Join-Path $taskPackages '7zr.exe') $taskDestination
if ($LASTEXITCODE -ne 0) { throw 'Portable Vulkan SDK extraction failed' }
Write-Output ('Vulkan build tools: ' + $taskDestination)
