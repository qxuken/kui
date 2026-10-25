# Installs the kui runner on Windows (ADR 0068): downloads the release's
# kui-<version>-win32-x64.zip, checks it against the release's SHA256SUMS,
# and puts kui.exe in %USERPROFILE%\.kui\bin. It changes no environment
# variable; it prints the PATH lines to run.
#
#   irm https://github.com/qxuken/kui/releases/download/v<version>/install.ps1 | iex
#   .\install.ps1                     the newest release
#   .\install.ps1 0.1.0-alpha.68      that one (a leading `v` is fine)
#
# Environment (what `irm | iex`, which takes no arguments, is steered by):
#   KUI_VERSION      the version, when no argument names one
#   KUI_INSTALL_DIR  where kui.exe goes (default %USERPROFILE%\.kui\bin)
#   KUI_MIRROR       a mirror with the release's layout, used instead of
#                    GitHub: <mirror>/v<version>/<asset>, and
#                    <mirror>/latest.txt holding the newest version for a
#                    run that names none
#
# "The newest release" is the newest the GitHub API lists, prereleases
# included: every kui release so far is an alpha, and GitHub's own
# `latest` skips prereleases. Windows PowerShell 5.1 and PowerShell 7 both
# run it. Errors are thrown, never `exit`: under `iex` an exit would close
# the user's own window.
param([string]$Version)

function Install-Kui {
    param([string]$Version)
    $ErrorActionPreference = 'Stop'
    # Invoke-WebRequest's progress bar slows a download severalfold on 5.1.
    $ProgressPreference = 'SilentlyContinue'
    # Windows PowerShell 5.1 may still default to TLS 1.0, which GitHub refuses.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'qxuken/kui'
    if (-not $Version) { $Version = $env:KUI_VERSION }
    $dir = if ($env:KUI_INSTALL_DIR) { $env:KUI_INSTALL_DIR } else { Join-Path $env:USERPROFILE '.kui\bin' }
    $mirror = if ($env:KUI_MIRROR) { $env:KUI_MIRROR.TrimEnd('/') } else { $null }

    # x64 only; Windows 11 on Arm runs the x64 build under emulation.
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        'AMD64' { }
        'ARM64' { Write-Host 'kui has no Windows on Arm build; installing the x64 one, which runs under emulation.' }
        default { throw "kui install: no kui build for a $arch Windows (there is win32-x64); build it from source: cargo build -p kui-cli --release in https://github.com/$repo" }
    }

    if (-not $Version) {
        try {
            if ($mirror) {
                # To a file: 5.1 hands .Content back as bytes for a mirror
                # that serves it as application/octet-stream.
                $latest = Join-Path ([IO.Path]::GetTempPath()) ("kui-latest-" + [Guid]::NewGuid().ToString('N') + '.txt')
                Invoke-WebRequest -UseBasicParsing "$mirror/latest.txt" -OutFile $latest
                $Version = (Get-Content -Raw $latest).Trim()
                Remove-Item -Force $latest -ErrorAction SilentlyContinue
            } else {
                $Version = (Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$repo/releases?per_page=1")[0].tag_name
            }
        } catch {
            throw "kui install: could not tell the newest kui version ($($_.Exception.Message)); name one: `$env:KUI_VERSION = '<version>'"
        }
        if (-not $Version) { throw "kui install: could not tell the newest kui version; name one: `$env:KUI_VERSION = '<version>'" }
    }
    $Version = $Version.Trim() -replace '^v', ''

    $asset = "kui-$Version-win32-x64.zip"
    $base = if ($mirror) { "$mirror/v$Version" } else { "https://github.com/$repo/releases/download/v$Version" }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("kui-install-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Write-Host "kui $Version for win32-x64 from $base"
        try {
            Invoke-WebRequest -UseBasicParsing "$base/$asset" -OutFile (Join-Path $tmp $asset)
        } catch {
            throw "kui install: could not download $base/$asset (is $Version a kui release, and one with runner binaries?): $($_.Exception.Message)"
        }
        Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS')

        # `<hex>  <name>`, or `<hex> *<name>` as sha256sum -b writes it.
        $want = $null
        foreach ($line in Get-Content (Join-Path $tmp 'SHA256SUMS')) {
            $f = $line.Trim() -split '\s+', 2
            if ($f.Count -eq 2 -and ($f[1] -eq $asset -or $f[1] -eq "*$asset")) { $want = $f[0].ToLowerInvariant(); break }
        }
        if (-not $want) { throw "kui install: SHA256SUMS has no line for $asset" }
        $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $asset)).Hash.ToLowerInvariant()
        if ($got -ne $want) { throw "kui install: checksum mismatch for ${asset}: expected $want, got $got; nothing was installed" }

        $x = Join-Path $tmp 'x'
        Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $x -Force
        $new = Join-Path $x 'kui.exe'
        if (-not (Test-Path $new)) { throw "kui install: $asset holds no kui.exe" }

        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $exe = Join-Path $dir 'kui.exe'
        # A running kui.exe cannot be overwritten but can be renamed: move
        # it aside, put the new one in its place, and drop the old one when
        # nothing holds it any more.
        $old = "$exe.old"
        if (Test-Path $old) { Remove-Item -Force $old -ErrorAction SilentlyContinue }
        if (Test-Path $exe) { Move-Item -Force $exe $old }
        Move-Item -Force $new $exe
        if (Test-Path $old) { Remove-Item -Force $old -ErrorAction SilentlyContinue }
        Unblock-File $exe -ErrorAction SilentlyContinue

        $shown = ''
        try { $shown = (& $exe -V 2>$null | Out-String).Trim() } catch { }
        if (-not $shown) { $shown = "kui $Version" }
        Write-Host "installed $shown at $exe"

        $onPath = ($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') }
        if (-not $onPath) {
            Write-Host ''
            Write-Host "$dir is not on your PATH. Add it for your user with:"
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', '$dir;' + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
            Write-Host 'and for this window with:'
            Write-Host "  `$env:Path = '$dir;' + `$env:Path"
        }
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

Install-Kui -Version $Version
