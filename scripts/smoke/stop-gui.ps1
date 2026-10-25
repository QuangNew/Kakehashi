$taskRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$ErrorActionPreference = 'Stop'
$taskMeta = Get-Content -LiteralPath (Join-Path $taskRoot '.cache/smoke/gui-process.json') -Raw | ConvertFrom-Json
$taskProcess = Get-Process -Id $taskMeta.pid -ErrorAction Stop
if ($taskProcess.Path -ne $taskMeta.path) { throw 'Test desktop process does not match the recorded executable' }
$taskChildren = @(Get-CimInstance Win32_Process -Filter "Name='llama-server.exe'" | Where-Object ParentProcessId -eq $taskMeta.pid | Select-Object -ExpandProperty ProcessId)
& node (Join-Path $PSScriptRoot 'gui-smoke.mjs') --quit
if ($LASTEXITCODE -ne 0) { throw 'Could not request desktop exit' }
if (-not $taskProcess.WaitForExit(8000)) { throw 'Test desktop did not exit normally' }
Start-Sleep -Milliseconds 800
foreach ($taskChildId in $taskChildren) {
    if (Get-Process -Id $taskChildId -ErrorAction SilentlyContinue) { throw ('Owned model server still running: ' + $taskChildId) }
}
Write-Output ('Normal exit verified; ' + $taskChildren.Count + ' owned model servers stopped')
