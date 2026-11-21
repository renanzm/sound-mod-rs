# Compila o Sound Mod. Nao precisa do Visual Studio: usa o Rust (toolchain GNU) com o linker
# LLVM que o proprio rustup instala.
#
# Uso: .\build.ps1             gera dist\SoundMod\SoundMod.exe (release, um unico .exe)
#      .\build.ps1 -Run        compila em modo debug e abre o app
#      .\build.ps1 -Test       roda os testes
#      .\build.ps1 -Test -Hardware   inclui o teste real com o VB-CABLE (toca um tom no cabo)
#      .\build.ps1 -Package    release + dist\SoundMod-v<versao>-windows-x64.zip (exe, manuais, licenca)
param([switch]$Run, [switch]$Test, [switch]$Hardware, [switch]$Package)

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
if (-not $Package) { return }

# ---- pacote da release: SoundMod.exe + manuais em HTML + licenca, num .zip ----
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$name = "SoundMod-v$version-windows-x64"
$pkg = Join-Path $PSScriptRoot "dist\package"
if (Test-Path $pkg) { Remove-Item $pkg -Recurse -Force }  # pasta temporaria do proprio build
New-Item -ItemType Directory $pkg | Out-Null
Copy-Item "target\release\SoundMod.exe" $pkg
Copy-Item LICENSE (Join-Path $pkg "LICENSE.txt")

$utf8 = New-Object System.Text.UTF8Encoding $false
$template = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "tools\manual-template.html"), $utf8)
$manuals = @(
    @{ Src = "docs\MANUAL.pt-BR.md"; Out = "Manual-pt-BR.html"; Lang = "pt-BR"; Img = "screenshot-pt.png"; Title = "Sound Mod - Manual de uso" },
    @{ Src = "docs\MANUAL.md";       Out = "Manual-en.html";    Lang = "en";    Img = "screenshot.png";    Title = "Sound Mod - User Manual" }
)
foreach ($m in $manuals) {
    # Markdown -> HTML pelo renderizador do GitHub (o mesmo da pagina do repositorio)
    $text = [IO.File]::ReadAllText((Join-Path $PSScriptRoot $m.Src), $utf8)
    $json = @{ text = $text; mode = "gfm"; context = "renanzm/sound-mod-rs" } | ConvertTo-Json -Compress
    $resp = Invoke-WebRequest -UseBasicParsing -Method Post -Uri "https://api.github.com/markdown" `
        -ContentType "application/json; charset=utf-8" -Body ([Text.Encoding]::UTF8.GetBytes($json)) `
        -Headers @{ "User-Agent" = "sound-mod-build"; "Accept" = "text/html" }
    $html = $utf8.GetString($resp.RawContentStream.ToArray())
    # ancoras dos titulos (o sumario aponta para elas), no mesmo formato do GitHub
    $html = [regex]::Replace($html, '<h([1-6]) dir="auto">(.*?)</h\1>', {
        param($x)
        $plain = [regex]::Replace($x.Groups[2].Value, '<[^>]+>', '')
        $slug = [regex]::Replace($plain.ToLowerInvariant(), '[^\p{L}\p{N} _-]', '') -replace ' ', '-'
        "<h$($x.Groups[1].Value) id=""$slug"" dir=""auto"">$($x.Groups[2].Value)</h$($x.Groups[1].Value)>"
    })
    # print embutido (o manual abre sem internet) e links entre os idiomas
    $png = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $PSScriptRoot "docs\$($m.Img)")))
    $html = $html.Replace("src=""$($m.Img)""", "src=""data:image/png;base64,$png""").Replace("href=""$($m.Img)""", "href=""#""")
    $html = $html.Replace('href="MANUAL.md"', 'href="Manual-en.html"').Replace('href="MANUAL.pt-BR.md"', 'href="Manual-pt-BR.html"')
    if ($m.Lang -eq "pt-BR") {
        # o GitHub escreve os titulos dos avisos em ingles. "Atencao" com acentos montado por
        # codigo: o Windows PowerShell 5.1 le este .ps1 (sem BOM) como ANSI
        $atencao = "Aten" + [char]0x00E7 + [char]0x00E3 + "o"
        $alerts = @{ Note = "Nota"; Tip = "Dica"; Important = "Importante"; Warning = $atencao; Caution = "Cuidado" }
        foreach ($k in $alerts.Keys) { $html = $html.Replace("</svg>$k</p>", "</svg>$($alerts[$k])</p>") }
    }
    $page = $template.Replace("{{LANG}}", $m.Lang).Replace("{{TITLE}}", $m.Title).Replace("{{VERSION}}", "v$version").Replace("{{BODY}}", $html)
    [IO.File]::WriteAllText((Join-Path $pkg $m.Out), $page, $utf8)
}

$zip = Join-Path $PSScriptRoot "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $pkg "*") -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
$exeHash = (Get-FileHash "target\release\SoundMod.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "Pacote: dist\$name.zip"
Write-Host "SHA-256 $name.zip: $hash"
Write-Host "SHA-256 SoundMod.exe: $exeHash"
