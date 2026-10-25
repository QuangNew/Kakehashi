$ErrorActionPreference = 'Stop'
$taskRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskApp = Join-Path $taskRoot 'out/windows/Kakehashi.exe'
if (-not (Test-Path -LiteralPath $taskApp)) { throw 'Desktop executable is not built yet' }
New-Item -ItemType Directory -Force -Path (Join-Path $taskRoot '.cache/smoke') | Out-Null
$taskProbe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 9227)
$taskProbe.Start()
$taskProbe.Stop()
$taskPreviousPath = $env:PATH
$taskPreviousArguments = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$taskPreviousLog = $env:RUST_LOG
try {
    $env:PATH = $env:SystemRoot + '\System32;' + $env:SystemRoot
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9227'
    $env:RUST_LOG = 'info'
    $taskProcess = Start-Process -FilePath $taskApp -WorkingDirectory ([IO.Path]::GetDirectoryName($taskApp)) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $taskRoot '.cache/smoke/gui.stdout.log') -RedirectStandardError (Join-Path $taskRoot '.cache/smoke/gui.stderr.log')
    @{ pid = $taskProcess.Id; path = $taskApp; started = (Get-Date).ToString('o') } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot '.cache/smoke/gui-process.json') -Encoding utf8
} finally {
    $env:PATH = $taskPreviousPath
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = $taskPreviousArguments
    $env:RUST_LOG = $taskPreviousLog
}
for ($taskAttempt = 0; $taskAttempt -lt 40; $taskAttempt++) {
    if ($taskProcess.HasExited) { throw ('Desktop exited with code ' + $taskProcess.ExitCode) }
    try {
        $taskTargets = Invoke-RestMethod -Uri http://127.0.0.1:9227/json -TimeoutSec 1
        if ($taskTargets.Count -gt 0) { Write-Output ('Desktop CDP ready; process ' + $taskProcess.Id); exit 0 }
    } catch { }
    Start-Sleep -Milliseconds 500
}
throw 'Desktop did not expose its webviews within the startup timeout'
