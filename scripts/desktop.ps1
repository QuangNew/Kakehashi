param(
    [ValidateSet('dev', 'build', 'test', 'smoke', 'benchmark')][string]$Mode = 'dev',
    [ValidateSet('auto', 'portable')][string]$CpuProfile = 'auto',
    [ValidateSet('auto', 'cpu', 'vulkan')][string]$WhisperBackend = 'auto',
    [string]$AudioPath,
    [ValidateRange(3, 3600)][int]$Seconds = 90
)
$ErrorActionPreference = 'Stop'
$taskRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $taskRoot
try {
    if ($Mode -eq 'benchmark') {
        if (-not $AudioPath) { throw 'Pass -AudioPath with a 16 kHz mono PCM16 WAV file.' }
        $taskBenchmarkAudio = (Resolve-Path -LiteralPath $AudioPath -ErrorAction Stop).Path
    }
    # A local MinGW build avoids a system-wide Visual Studio installation.
    $taskPortable = Join-Path $taskRoot '.native-tools'
    if (-not (Test-Path -LiteralPath (Join-Path $taskPortable 'cmake/cmake/data/bin/cmake.exe')) -or
        -not (Get-ChildItem -Path (Join-Path $taskPortable 'libclang') -Filter libclang.dll -Recurse -ErrorAction SilentlyContinue) -or
        -not (Get-ChildItem -Path (Join-Path $taskPortable 'ninja') -Filter ninja.exe -Recurse -ErrorAction SilentlyContinue)) {
        New-Item -ItemType Directory -Force -Path (Join-Path $taskPortable 'packages') | Out-Null
        & python -m pip download --no-deps --only-binary=:all: --dest (Join-Path $taskPortable 'packages') cmake==3.31.6 ninja==1.13.2 libclang==18.1.1
        if ($LASTEXITCODE -ne 0) { throw 'Portable build-tool download failed' }
        & python (Join-Path $PSScriptRoot 'unpack-native-tools.py')
        if ($LASTEXITCODE -ne 0) { throw 'Portable build-tool extraction failed' }
    }
    if ($WhisperBackend -eq 'vulkan' -or ($WhisperBackend -eq 'auto' -and (Test-Path -LiteralPath (Join-Path $env:SystemRoot 'System32/vulkan-1.dll')))) {
        & (Join-Path $PSScriptRoot 'setup-vulkan.ps1')
    }
    . (Join-Path $PSScriptRoot 'native-env.ps1') -CpuProfile $CpuProfile -WhisperBackend $WhisperBackend
    if ($Mode -in @('dev', 'build')) {
        & node (Join-Path $PSScriptRoot 'fetch-onnxruntime.mjs')
        if ($LASTEXITCODE -ne 0) { throw 'ONNX Runtime preparation failed' }
        $taskNativeDebug = Join-Path $taskRoot 'src-tauri/target/debug'
        New-Item -ItemType Directory -Force -Path $taskNativeDebug | Out-Null
        foreach ($taskOrtDll in Get-ChildItem -LiteralPath (Join-Path $taskRoot 'src-tauri/ort') -Filter '*.dll') {
            $taskOrtDest = Join-Path $taskNativeDebug $taskOrtDll.Name
            if (-not (Test-Path -LiteralPath $taskOrtDest) -or
                (Get-FileHash -LiteralPath $taskOrtDest).Hash -ne (Get-FileHash -LiteralPath $taskOrtDll.FullName).Hash) {
                Copy-Item -LiteralPath $taskOrtDll.FullName -Destination $taskOrtDest -Force
            }
        }
    }
    $env:RUSTUP_TOOLCHAIN = 'stable-x86_64-pc-windows-gnu'
    Push-Location src-tauri
    try {
        # test/build/run already compile the code. A preceding cargo check can
        # build a second copy of the native Vulkan backend for its metadata graph.
        if ($Mode -eq 'dev') {
            & cargo check --locked @taskWhisperFeatures
            if ($LASTEXITCODE -ne 0) { throw 'Rust check failed' }
        }
        Sync-WhisperGnuLibraries | Out-Null
        if ($Mode -eq 'test') {
            & cargo test --locked --lib @taskWhisperFeatures
            if ($LASTEXITCODE -ne 0 -and (Sync-WhisperGnuLibraries) -gt 0) { & cargo test --locked --lib @taskWhisperFeatures }
        }
        elseif ($Mode -eq 'smoke') {
            & cargo run --locked --example local_model_smoke @taskWhisperFeatures
            if ($LASTEXITCODE -ne 0 -and (Sync-WhisperGnuLibraries) -gt 0) { & cargo run --locked --example local_model_smoke @taskWhisperFeatures }
        }
        elseif ($Mode -eq 'benchmark') {
            & cargo run --locked --example stt_realtime_bench @taskWhisperFeatures -- $taskBenchmarkAudio $Seconds --translate --dual
            if ($LASTEXITCODE -ne 0 -and (Sync-WhisperGnuLibraries) -gt 0) { & cargo run --locked --example stt_realtime_bench @taskWhisperFeatures -- $taskBenchmarkAudio $Seconds --translate --dual }
        }
        if ($Mode -in @('test', 'smoke', 'benchmark') -and $LASTEXITCODE -ne 0) { throw ('Native ' + $Mode + ' failed') }
    } finally { Pop-Location }
    if ($Mode -eq 'dev') { & pnpm.cmd tauri dev @taskWhisperFeatures }
    if ($Mode -eq 'build') {
        function Copy-DesktopRuntimeFile([string]$Source, [string]$Destination) {
            # Windows may keep a loaded DLL locked. An identical staged file is
            # already usable and does not need to be overwritten on every build.
            if (Test-Path -LiteralPath $Destination -PathType Leaf) {
                $taskSourceInfo = Get-Item -LiteralPath $Source
                $taskDestinationInfo = Get-Item -LiteralPath $Destination
                if ($taskSourceInfo.Length -eq $taskDestinationInfo.Length -and
                    (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -eq
                    (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash) { return }
            }
            Copy-Item -LiteralPath $Source -Destination $Destination -Force -ErrorAction Stop
        }
        & pnpm.cmd tauri build --debug --no-bundle @taskWhisperFeatures
        if ($LASTEXITCODE -ne 0 -and (Sync-WhisperGnuLibraries) -gt 0) { & pnpm.cmd tauri build --debug --no-bundle @taskWhisperFeatures }
        if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
        $taskOutputDir = Join-Path $taskRoot 'out/windows'
        New-Item -ItemType Directory -Force -Path $taskOutputDir | Out-Null
        Copy-Item -LiteralPath src-tauri/target/debug/nexq.exe -Destination (Join-Path $taskOutputDir 'Kakehashi.exe') -Force
        Copy-DesktopRuntimeFile (Join-Path $taskRoot 'src-tauri/target/debug/WebView2Loader.dll') (Join-Path $taskOutputDir 'WebView2Loader.dll')
        foreach ($taskOrtDll in Get-ChildItem -LiteralPath (Join-Path $taskRoot 'src-tauri/ort') -Filter '*.dll') {
            Copy-DesktopRuntimeFile $taskOrtDll.FullName (Join-Path $taskOutputDir $taskOrtDll.Name)
        }
        $taskCompilerDir = [System.IO.Path]::GetDirectoryName($env:TVNJP_CXX_PATH)
        $taskDlls = @('libstdc++-6.dll', 'libgcc_s_seh-1.dll', 'libwinpthread-1.dll', 'liblzma-5.dll')
        if ($taskUseAvx2) { $taskDlls += 'libgomp-1.dll' }
        foreach ($taskDll in $taskDlls) {
            Copy-DesktopRuntimeFile (Join-Path $taskCompilerDir $taskDll) (Join-Path $taskOutputDir $taskDll)
        }
        Write-Output ('Desktop app: ' + (Join-Path $taskOutputDir 'Kakehashi.exe'))
    }
} finally { Pop-Location }
