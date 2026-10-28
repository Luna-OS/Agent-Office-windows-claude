# OpenAgenticOffice installieren (Windows) und starten.
#
#   irm https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.ps1 | iex
#
# Es wird kein Node.js gebraucht: das Archiv enthält den fertigen Server (open-agentic-office.exe),
# den gebauten Browser-Client (dist\) und die Agenten-CLI (office-agenten.exe).
#
# Einstellungen (Umgebungsvariablen, vor dem Aufruf setzen):
#   $env:OFFICE_VERSION       eine bestimmte Version installieren, z. B. 0.1.12 (Standard: die neueste)
#   $env:OFFICE_INSTALL_ONLY  auf 1 setzen: nur installieren, das Office nicht starten
#   $env:OFFICE_INSTALL_DIR   Zielordner (Standard: %LOCALAPPDATA%\open-agentic-office; die Programme liegen in bin\)
#   $env:OFFICE_ARCHIV        ein lokales .zip statt eines Downloads verwenden (für Tests)
#   $env:OFFICE_REPO          GitHub-Repository (Standard: Luna-OS/OpenAgenticOffice)
#
# Alles nach dem Start (Port, Passwort, Ordner) steht in docs/configuration.md.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = if ($env:OFFICE_REPO) { $env:OFFICE_REPO } else { 'Luna-OS/OpenAgenticOffice' }
$ziel = if ($env:OFFICE_INSTALL_DIR) { $env:OFFICE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'open-agentic-office' }
$bin = Join-Path $ziel 'bin'
$asset = 'open-agentic-office-windows-x64.zip'

function Info($text) { Write-Host "==> $text" }

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("oao-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $archiv = Join-Path $tmp 'archiv.zip'
    if ($env:OFFICE_ARCHIV) {
        Info "Verwende das lokale Archiv $($env:OFFICE_ARCHIV)"
        Copy-Item $env:OFFICE_ARCHIV $archiv
    } else {
        if ($env:OFFICE_VERSION) {
            $url = "https://github.com/$repo/releases/download/v$($env:OFFICE_VERSION.TrimStart('v'))/$asset"
        } else {
            $url = "https://github.com/$repo/releases/latest/download/$asset"
        }
        Info "Lade $url"
        try { Invoke-WebRequest -Uri $url -OutFile $archiv -UseBasicParsing }
        catch { throw "Der Download ist fehlgeschlagen (gibt es diese Version?): $($_.Exception.Message)" }
    }

    Info "Installiere nach $bin"
    $auspacken = Join-Path $tmp 'auspacken'
    Expand-Archive -Path $archiv -DestinationPath $auspacken -Force
    $server = Get-ChildItem -Path $auspacken -Recurse -Filter 'open-agentic-office.exe' | Select-Object -First 1
    if (-not $server) { throw 'Das Archiv enthält keinen Server (open-agentic-office.exe).' }
    # Läuft das Office noch aus einer früheren Installation, lässt sich die Datei nicht ersetzen.
    Get-Process -Name 'open-agentic-office' -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$bin*" } | Stop-Process -Force
    if (Test-Path $bin) { Remove-Item $bin -Recurse -Force }
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    Copy-Item -Path (Join-Path $server.DirectoryName '*') -Destination $bin -Recurse -Force
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# PATH des Benutzers ergänzen
$benutzerPfad = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($benutzerPfad -split ';') -notcontains $bin) {
    [Environment]::SetEnvironmentVariable('Path', ($benutzerPfad.TrimEnd(';') + ";$bin"), 'User')
    Info 'PATH ergänzt (ein neues Terminal öffnen, damit er gilt)'
}
if (($env:Path -split ';') -notcontains $bin) { $env:Path = "$env:Path;$bin" }

# Voraussetzungen prüfen (nur Hinweise)
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host 'Hinweis: git fehlt. Das Office braucht es für Projekte und Worktrees: winget install --id Git.Git -e'
}
if (Get-Command gh -ErrorAction SilentlyContinue) {
    gh auth status *> $null
    if ($LASTEXITCODE -ne 0) { Write-Host "Hinweis: die GitHub CLI ist nicht angemeldet. Mit 'gh auth login' kommst du an deine Repositorys." }
} else {
    Write-Host 'Hinweis: die GitHub CLI (gh) fehlt: winget install --id GitHub.cli -e. Ohne sie gibt es keine Repository-Liste, Issues und Pull Requests.'
}
if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
    Write-Host 'Hinweis: kein Agent-CLI gefunden (z. B. Claude Code: irm https://claude.ai/install.ps1 | iex).'
}

Info 'Fertig. Starten: open-agentic-office'
if ($env:OFFICE_INSTALL_ONLY -eq '1') { return }
& (Join-Path $bin 'open-agentic-office.exe')
