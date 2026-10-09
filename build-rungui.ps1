$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
    if (!(Get-Command cmake -ErrorAction SilentlyContinue)) {
        $vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
        if (Test-Path -LiteralPath $vswhere) {
            $installation=& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
            $cmake=Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
            $env:PATH=$cmake+';'+$env:PATH
        }
    }
    if (!$env:LIBCLANG_PATH) {
        $llvm=Join-Path $env:ProgramFiles 'LLVM\bin'
        if (Test-Path -LiteralPath (Join-Path $llvm 'libclang.dll')) {$env:LIBCLANG_PATH=$llvm}
    }
    cargo build --release --locked --no-default-features --features rungui-ui --bin minuter-rungui
    if ($LASTEXITCODE -ne 0) {throw 'Release build failed'}
    New-Item -ItemType Directory -Force experimental-dist | Out-Null
    Copy-Item -LiteralPath target\release\minuter-rungui.exe -Destination experimental-dist\Minuter.exe -Force
    $files=Get-ChildItem -LiteralPath experimental-dist -File
    if ($files.Count -ne 1 -or $files[0].Name -ne 'Minuter.exe') {throw 'Distribution folder must contain only Minuter.exe'}
    Write-Output "Built $($files[0].FullName)"
} finally {Pop-Location}
