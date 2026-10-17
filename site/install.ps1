# OpenCue installer for Windows.
#
#   irm https://lautaro005.github.io/opencue/install.ps1 | iex
#
# Downloads the latest installer from GitHub, installs OpenCue for your user (no admin) and adds
# the `opencue` command to your PATH. Uninstall it from Settings → Apps like any other app.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repo = 'Lautaro005/opencue'

Write-Host '› Looking up the latest release'
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ Accept = 'application/vnd.github+json' }
$asset = $release.assets | Where-Object { $_.name -like '*x64-setup.exe' } | Select-Object -First 1
if (-not $asset) { throw "The latest release ($($release.tag_name)) has no Windows installer yet." }

$setup = Join-Path $env:TEMP $asset.name
Write-Host "› Downloading OpenCue $($release.tag_name)"
Invoke-WebRequest $asset.browser_download_url -OutFile $setup -UseBasicParsing
Write-Host '› Installing'
Start-Process $setup -ArgumentList '/S' -Wait
Remove-Item $setup -ErrorAction SilentlyContinue

$dir = @("$env:LOCALAPPDATA\OpenCue", "$env:LOCALAPPDATA\Programs\OpenCue") |
  Where-Object { Test-Path (Join-Path $_ 'opencue.exe') } | Select-Object -First 1
if (-not $dir) { throw 'OpenCue was installed, but opencue.exe was not found.' }

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (($userPath -split ';') -contains $dir)) {
  [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir"), 'User')
  $env:Path += ";$dir"
}
Write-Host "› Installed OpenCue $($release.tag_name). Start it with: opencue"
