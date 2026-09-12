param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$moteRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $moteRoot
try {
    if (-not $SkipBuild) {
        & cargo build --release -p mote-app
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    $moteVersion = (Select-String -LiteralPath (Join-Path $moteRoot 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
    $motePackageName = "Mote-$moteVersion-windows-x64"
    $motePackageDirectory = Join-Path $moteRoot "target/package/$motePackageName"
    New-Item -ItemType Directory -Path $motePackageDirectory -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $moteRoot 'target/release/mote.exe') -Destination (Join-Path $motePackageDirectory 'Mote.exe') -Force
    Copy-Item -LiteralPath (Join-Path $moteRoot 'LICENSE') -Destination $motePackageDirectory -Force
    Copy-Item -LiteralPath (Join-Path $moteRoot 'docs/QUICKSTART.md') -Destination $motePackageDirectory -Force
    $moteZip = Join-Path $moteRoot "target/package/$motePackageName.zip"
    Compress-Archive -LiteralPath $motePackageDirectory -DestinationPath $moteZip -Force
    Get-FileHash -LiteralPath $moteZip -Algorithm SHA256 | Format-List
    Write-Output "Portable package: $moteZip"
} finally { Pop-Location }
