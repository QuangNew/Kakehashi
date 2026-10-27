# Dot-source this file before cargo/pnpm tauri when using the existing MinGW toolchain.
param(
    [ValidateSet('auto', 'portable')][string]$CpuProfile = 'auto',
    [ValidateSet('auto', 'cpu', 'vulkan')][string]$WhisperBackend = 'auto'
)
$taskWorkspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskTools = Join-Path $taskWorkspace '.native-tools'
$taskClang = Get-ChildItem -LiteralPath (Join-Path $taskTools 'libclang') -Filter libclang.dll -Recurse | Select-Object -First 1
$taskNinja = Get-ChildItem -LiteralPath (Join-Path $taskTools 'ninja') -Filter ninja.exe -Recurse | Select-Object -First 1
if (-not $taskClang -or -not $taskNinja) { throw 'Prepare portable libclang, CMake and Ninja in .native-tools first (see docs/guides/local-models.vi.md).' }
$env:LIBCLANG_PATH = $taskClang.DirectoryName
$env:PATH = (Join-Path $taskTools 'cmake/cmake/data/bin') + ';' + $taskNinja.DirectoryName + ';' + $env:PATH
$env:CMAKE_GENERATOR = 'Ninja'
$env:WHISPER_BUILD_TESTS = 'OFF'
$env:WHISPER_BUILD_EXAMPLES = 'OFF'
$env:WHISPER_BUILD_SERVER = 'OFF'
$env:CARGO_BUILD_JOBS = '4'
$env:TVNJP_CXX_PATH = (Get-Command g++.exe -ErrorAction Stop).Source
$env:WHISPER_DONT_GENERATE_BINDINGS = $null
$taskCompilerIncludes = (& $env:TVNJP_CXX_PATH -print-file-name=include).Trim().Replace('\', '/')
$taskSystemIncludes = [System.IO.Path]::GetFullPath((Join-Path ([System.IO.Path]::GetDirectoryName($env:TVNJP_CXX_PATH)) '../include')).Replace('\', '/')
$env:BINDGEN_EXTRA_CLANG_ARGS = '--target=x86_64-w64-windows-gnu -isystem "' + $taskCompilerIncludes + '" -isystem "' + $taskSystemIncludes + '"'
$taskCpuProbe = Join-Path $taskTools 'cpu-features.exe'
$taskCpuSource = Join-Path $PSScriptRoot 'cpu-features.rs'
if (-not (Test-Path -LiteralPath $taskCpuProbe) -or (Get-Item -LiteralPath $taskCpuProbe).LastWriteTimeUtc -lt (Get-Item -LiteralPath $taskCpuSource).LastWriteTimeUtc) {
    & rustc +stable-x86_64-pc-windows-gnu $taskCpuSource -o $taskCpuProbe
    if ($LASTEXITCODE -ne 0) { throw 'Could not detect native CPU features' }
}
$taskCpuFeatures = (& $taskCpuProbe).Trim()
$taskUseAvx2 = $CpuProfile -eq 'auto' -and $taskCpuFeatures -eq 'avx2'
$env:KAKEHASHI_CPU_PROFILE = if ($taskUseAvx2) { 'avx2' } else { 'portable' }
$taskFeatureNames = @()
if ($taskUseAvx2) { $taskFeatureNames += 'cpu-optimized' }
$taskVulkanSdk = Join-Path $taskTools 'vulkan'
$taskHasVulkanSdk = (Test-Path -LiteralPath (Join-Path $taskVulkanSdk 'Bin/glslc.exe')) -and (Test-Path -LiteralPath (Join-Path $taskVulkanSdk 'Lib/vulkan-1.lib'))
if ($WhisperBackend -eq 'vulkan' -and -not $taskHasVulkanSdk) { throw 'Run scripts/setup-vulkan.ps1 to prepare the portable Vulkan build tools.' }
$taskUseVulkan = $WhisperBackend -ne 'cpu' -and $taskHasVulkanSdk
if ($taskUseVulkan) {
    $env:VULKAN_SDK = $taskVulkanSdk
    $env:PATH = (Join-Path $taskVulkanSdk 'Bin') + ';' + $env:PATH
    $taskFeatureNames += 'gpu-vulkan'
}
$taskWhisperFeatures = if ($taskFeatureNames.Count -gt 0) { @('--features', ($taskFeatureNames -join ',')) } else { @() }
$env:KAKEHASHI_WHISPER_BACKEND = if ($taskUseVulkan) { 'vulkan' } else { 'cpu' }
$env:GGML_VULKAN = if ($taskUseVulkan) { 'ON' } else { 'OFF' }
$env:GGML_NATIVE = 'OFF'
$env:GGML_SSE42 = 'ON'
foreach ($taskFeature in @('GGML_AVX', 'GGML_AVX2', 'GGML_FMA', 'GGML_F16C', 'GGML_BMI2')) {
    [Environment]::SetEnvironmentVariable($taskFeature, $(if ($taskUseAvx2) { 'ON' } else { 'OFF' }), 'Process')
}
$env:GGML_AVX_VNNI = 'OFF'
$env:GGML_AVX512 = 'OFF'
# bindgen tracks this variable. Including the profile forces whisper-rs-sys to
# rerun CMake when profiles change; GGML_* alone are not tracked by that crate.
$env:BINDGEN_EXTRA_CLANG_ARGS += ' -DKAKEHASHI_CPU_' + $env:KAKEHASHI_CPU_PROFILE.ToUpperInvariant() + '=1 -DKAKEHASHI_BACKEND_' + $env:KAKEHASHI_WHISPER_BACKEND.ToUpperInvariant() + '=1'
Write-Output ('Whisper backend: ' + $env:KAKEHASHI_WHISPER_BACKEND + '; CPU fallback: ' + $env:KAKEHASHI_CPU_PROFILE)
$taskWrapper = Join-Path $taskTools 'gnu-cxx.exe'
$taskWrapperSource = Join-Path $PSScriptRoot 'gnu-cxx.rs'
if (-not (Test-Path -LiteralPath $taskWrapper) -or (Get-Item -LiteralPath $taskWrapper).LastWriteTimeUtc -lt (Get-Item -LiteralPath $taskWrapperSource).LastWriteTimeUtc) {
    & rustc +stable-x86_64-pc-windows-gnu $taskWrapperSource -o $taskWrapper
    if ($LASTEXITCODE -ne 0) { throw 'Could not build the MinGW C++ adapter' }
}
$env:CXX = $taskWrapper
$env:CMAKE_CXX_COMPILER = $taskWrapper

function Sync-WhisperGnuLibraries {
    # ggml's Windows CMake rules omit "lib" on .a files; rustc's GNU linker expects it.
    $taskBuildDirectory = Join-Path $taskWorkspace 'src-tauri/target'
    if (-not (Test-Path -LiteralPath $taskBuildDirectory)) { return 0 }
    $taskRepairs = 0
    foreach ($taskProfile in @('debug', 'release')) {
        $taskBuildCache = Join-Path $taskBuildDirectory ($taskProfile + '/build')
        if (-not (Test-Path -LiteralPath $taskBuildCache)) { continue }
        foreach ($taskPackage in (Get-ChildItem -LiteralPath $taskBuildCache -Filter 'whisper-rs-sys-*' -Directory)) {
            foreach ($taskLibrary in @('ggml', 'ggml-base', 'ggml-cpu', 'ggml-vulkan')) {
                $taskSourceLibrary = Join-Path $taskPackage.FullName ('out/lib/' + $taskLibrary + '.a')
                if (Test-Path -LiteralPath $taskSourceLibrary) {
                    $taskDestinationLibrary = Join-Path $taskPackage.FullName ('out/lib' + $taskLibrary + '.a')
                    $taskSourceInfo = Get-Item -LiteralPath $taskSourceLibrary
                    $taskDestinationInfo = Get-Item -LiteralPath $taskDestinationLibrary -ErrorAction SilentlyContinue
                    if (-not $taskDestinationInfo -or $taskSourceInfo.Length -ne $taskDestinationInfo.Length -or $taskSourceInfo.LastWriteTimeUtc -ne $taskDestinationInfo.LastWriteTimeUtc) {
                        Copy-Item -LiteralPath $taskSourceLibrary -Destination $taskDestinationLibrary -Force
                        $taskRepairs++
                    }
                }
            }
        }
    }
    return $taskRepairs
}
Sync-WhisperGnuLibraries | Out-Null
