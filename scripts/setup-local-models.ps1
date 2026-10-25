param(
    [string]$ModelsRoot = (Join-Path $PSScriptRoot '../models'),
    [string]$RuntimeRoot = (Join-Path $PSScriptRoot '../runtimes/llama'),
    [switch]$WithCopilot,
    [switch]$WithEmbeddings,
    [switch]$RuntimeOnly
)
$ErrorActionPreference = 'Stop'
$taskManifest = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'models.lock.json') | ConvertFrom-Json
$ModelsRoot = [System.IO.Path]::GetFullPath($ModelsRoot)
$RuntimeRoot = [System.IO.Path]::GetFullPath($RuntimeRoot)
$taskCache = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../.cache/model-downloads'))
New-Item -ItemType Directory -Force -Path $ModelsRoot,$RuntimeRoot,$taskCache | Out-Null

function Get-VerifiedFile([string]$Url, [string]$Destination, [string]$ExpectedHash) {
    if (Test-Path -LiteralPath $Destination) {
        if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant() -eq $ExpectedHash) {
            Write-Output ('Verified: ' + $Destination)
            return
        }
        throw ('Existing file has a different checksum: ' + $Destination + '. Move it aside before downloading the pinned version.')
    }
    New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($Destination)) | Out-Null
    $partial = $Destination + '.download'
    Write-Output ('Downloading: ' + $Destination)
    for ($attempt = 1; $attempt -le 6; $attempt++) {
        & curl.exe --fail --location --retry 3 --connect-timeout 20 --continue-at - --silent --show-error --output $partial $Url
        if ($LASTEXITCODE -eq 0) { break }
        if ($attempt -eq 6) { throw ('Download interrupted. Run this command again to resume: ' + $Destination) }
        Write-Output ('Connection interrupted; resuming download (attempt ' + ($attempt + 1) + ')')
        Start-Sleep -Seconds 2
    }
    $actualHash = (Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $ExpectedHash) { throw ('Checksum mismatch: ' + $Destination + '. The partial file was kept for inspection.') }
    Move-Item -LiteralPath $partial -Destination $Destination
    Write-Output ('Ready: ' + $Destination)
}

$taskArchive = Join-Path $taskCache ('llama-' + $taskManifest.runtime.version + '-vulkan-x64.zip')
Get-VerifiedFile $taskManifest.runtime.url $taskArchive $taskManifest.runtime.sha256
$taskRuntimeInfoPath = Join-Path $RuntimeRoot 'runtime.json'
$taskInstalled = if (Test-Path -LiteralPath $taskRuntimeInfoPath) { Get-Content -Raw -LiteralPath $taskRuntimeInfoPath | ConvertFrom-Json } else { $null }
if ($taskInstalled.sha256 -ne $taskManifest.runtime.sha256 -or -not (Test-Path -LiteralPath (Join-Path $RuntimeRoot 'llama-server.exe'))) {
    Expand-Archive -LiteralPath $taskArchive -DestinationPath $RuntimeRoot -Force
}
$taskServer = Get-ChildItem -LiteralPath $RuntimeRoot -Filter llama-server.exe -Recurse | Select-Object -First 1
if (-not $taskServer) { throw 'Runtime archive did not contain llama-server.exe' }
$taskManifest.runtime | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $RuntimeRoot 'runtime.json') -Encoding utf8

if (-not $RuntimeOnly) {
    foreach ($taskModel in $taskManifest.models) {
        if ($taskModel.role -eq 'copilot' -and -not $WithCopilot) { continue }
        if ($taskModel.role -eq 'embeddings' -and -not $WithEmbeddings) { continue }
        $taskUrl = 'https://huggingface.co/' + $taskModel.repo + '/resolve/' + $taskModel.revision + '/' + $taskModel.file
        Get-VerifiedFile $taskUrl (Join-Path $ModelsRoot $taskModel.path) $taskModel.sha256
    }
}

Write-Output ('Models folder: ' + $ModelsRoot)
Write-Output ('Runtime executable: ' + $taskServer.FullName)
Write-Output 'Open Settings > Local Models, then apply the Vietnamese-Japanese preset.'
