# Compila o Sound Mod. Nao precisa do Visual Studio: usa o Rust (toolchain GNU) com o linker
# LLVM que o proprio rustup instala.
#
# Uso: .\build.ps1             gera dist\SoundMod\SoundMod.exe (release, um unico .exe)
#      .\build.ps1 -Run        compila em modo debug e abre o app
#      .\build.ps1 -Test       roda os testes
#      .\build.ps1 -Test -Hardware   inclui o teste real com o VB-CABLE (toca um tom no cabo)
param([switch]$Run, [switch]$Test, [switch]$Hardware)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

# ---- Rust ----
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Test-Path (Join-Path $cargoBin "cargo.exe")) {
        $env:PATH = "$cargoBin;$env:PATH"
    } else {
        throw "Rust nao encontrado. Instale o rustup (https://rustup.rs) e rode de novo."
    }
}
# o rust-toolchain.toml pede o stable GNU + llvm-tools; o rustup instala na primeira chamada
$sysroot = (& rustc --print sysroot).Trim()
if ($LASTEXITCODE -ne 0) { throw "rustc falhou" }
$llvmBin = Join-Path $sysroot "lib\rustlib\x86_64-pc-windows-gnu\bin"

# ---- dlltool ----
# As crates windows* importam funcoes do Windows por "raw-dylib", que exige um dlltool.
# O llvm-ar do llvm-tools vira o llvm-dlltool quando o nome do executavel contem "dlltool".
$tools = Join-Path $PSScriptRoot "target\tools"
New-Item -ItemType Directory -Force $tools | Out-Null
$dlltool = Join-Path $tools "dlltool.exe"
$llvmAr = Join-Path $llvmBin "llvm-ar.exe"
if (-not (Test-Path $llvmAr)) { throw "llvm-ar nao encontrado: rode 'rustup component add llvm-tools'" }
if (-not (Test-Path $dlltool) -or (Get-Item $llvmAr).LastWriteTime -gt (Get-Item $dlltool).LastWriteTime) {
    Copy-Item $llvmAr $dlltool -Force
}
$env:PATH = "$tools;$env:PATH"

if ($Test) {
    if ($Hardware) { cargo test --release -- --include-ignored --nocapture } else { cargo test --release }
    if ($LASTEXITCODE -ne 0) { throw "testes falharam" }
    return
}
if ($Run) {
    cargo run
    return
}

if (Get-Process SoundMod -ErrorAction SilentlyContinue) {
    throw "Feche o Sound Mod antes de compilar (dist\SoundMod\SoundMod.exe esta em uso)."
}
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "a compilacao falhou" }

# os dados do usuario ficam em %APPDATA%\SoundMod: aqui so o .exe e trocado
$dist = Join-Path $PSScriptRoot "dist\SoundMod"
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item "target\release\SoundMod.exe" $dist -Force
Write-Host "Pronto: dist\SoundMod\SoundMod.exe"
