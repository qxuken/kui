# The half of Windows that `cargo test --workspace` cannot reach: a real
# window, on the real GPU, driven by the real event loop.
#
#   pwsh scripts/smoke-windows.ps1              # every windowed example
#   pwsh scripts/smoke-windows.ps1 -Frames 300  # longer, for pacing bugs
#   pwsh scripts/smoke-windows.ps1 -Only fragment,enter_exit
#   pwsh scripts/smoke-windows.ps1 -Dev         # the profile `cargo run` uses
#   pwsh scripts/smoke-windows.ps1 -Base light  # one theme base only
#
# Why this exists. The smoke job in .forgejo/workflows/smoke.yml runs
# `cargo test --workspace` natively, which proves the platform code compiles
# and links against the real SDK. It does not open a window and never
# touches a GPU, so the whole of `kui-wgpu` - surfaces, swapchains,
# pipelines, and everything wgpu validates about them - is uncovered on the
# platform kui ships to most. Three crashes found in one round on
# 2026-09-08, all invisible to every headless test in the repo:
#
#   * `fragments` panicked on its first paint. `uniform_align` came from the
#     *adapter*, which reports 64 on DX12, while the device was opened with
#     `Limits::default()` and its 256; the second fragment of a frame then
#     sat at a dynamic offset validation refused. macOS never saw it because
#     Metal's adapter reports 256 and the two agreed.
#   * A resize handed `Surface::configure` a height of 32711, which is
#     larger than the device's maximum texture dimension, which panics
#     inside wgpu.
#   * `windows_anim`'s timer: an animating window dragged by its title bar
#     lurched, because every frame the runner asked for blocked on vsync
#     inside Windows' modal move loop.
#
# How it works. `KUI_SMOKE_FRAMES=n` (crates/kui/src/lib.rs) makes the
# runner quit once the main window has presented n frames, so an example is
# a self-terminating check: exit 0 means it drew n frames and shut down,
# and anything else - a wgpu validation panic, a device loss, a hang - is a
# failure with the stderr to read. A frame only counts once a present has
# succeeded, so an example that opens a window and never paints runs out
# the timeout rather than passing quietly.
#
# Every example runs inside the devtools (examples/devtools, ADR 0021), so
# `--light` and `--dark` pin the theme base without the example knowing:
# each one is opened twice, once per base, and a literal colour that reads
# on one base and not the other is opened on both, every run.
#
# The runner honours that variable in a dev build, and in any build asking
# for `--features smoke`, which is what this script passes when it is not
# smoking dev: an app you ship should not close its own window because
# something in the environment it was launched from set a variable its
# author never asked about. `-NoBuild` on a release target/ built without
# the feature is therefore 14 timeouts, not 14 passes.
#
# What it does not cover: anything needing a human. The chrome behaviours
# in windows_nc.rs (snap layouts, caption menus, resize borders) and the
# UIA bridge in access_bridge.rs still have no automated check on any
# platform - see P8. Nor does it judge what was drawn; it judges that
# drawing it did not fail. `cargo test --workspace` is still what checks
# the pixels, through the conformance corpus.
#
# Exit codes: 0 every example passed, 1 at least one failed.

[CmdletBinding()]
param(
    # Frames each example must present before it is allowed to quit. 120 is
    # ~0.5 s at 240 Hz and enough for a first paint, a resize and a few
    # animation frames; raise it when hunting something intermittent.
    [int]$Frames = 120,
    # Seconds an example gets before it is called hung.
    [int]$TimeoutSec = 60,
    # Only these examples, by name. Empty means all of them.
    [string[]]$Only = @(),
    # Build and run the dev profile instead of release - the one
    # `cargo run --example x` gives you, and so the one a report is
    # usually about.
    [switch]$Dev,
    # Skip the build and run whatever is already in target/.
    [switch]$NoBuild,
    # The theme bases to open each example on; both by default.
    [string[]]$Base = @('light', 'dark')
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# Every example in crates/kui, read from its manifest so a new one is
# smoked the day it is added. All of them open a window; the ones that do
# not (`conformance-dump`, `bench`) and the two `panel` hosts for an
# extension built by examples/c/build.ps1 are other crates' examples.
$examples = @(((cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).packages |
    Where-Object name -eq 'kui').targets |
    Where-Object { $_.kind -contains 'example' } |
    ForEach-Object name | Sort-Object)
# `pwsh -File` hands parameters over as plain strings, so `-Only a,b` arrives
# as one element rather than two; split it back rather than making the
# documented spelling depend on how the script was invoked.
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Base = @($Base | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($Only.Count -gt 0) {
    $unknown = $Only | Where-Object { $examples -notcontains $_ }
    if ($unknown) { throw "no such example: $($unknown -join ', ')" }
    $examples = $Only
}

$cargoProfile = if ($Dev) { 'dev' } else { 'release' }
# Where cargo puts the `dev` profile is `debug`; the one mapping.
$profileDir = if ($Dev) { 'debug' } else { 'release' }
# `KUI_SMOKE_FRAMES` is honoured unasked only where debug assertions are on,
# so that a shipped app cannot be closed by a stray variable in the
# environment it was launched from. A release round asks for it by name.
$features = if ($Dev) { @() } else { @('--features', 'smoke') }

if (-not $NoBuild) {
    Write-Host "building $profileDir examples..." -ForegroundColor Cyan
    & cargo build --profile $cargoProfile -p kui --examples @features
    if ($LASTEXITCODE -ne 0) { throw "build failed" }
}

Write-Host ""
Write-Host ("smoke: {0} examples x ({1}), {2} frames each, {3} profile" -f $examples.Count, ($Base -join ' '), $Frames, $profileDir)
Write-Host ""

$failed = @()
$env:KUI_SMOKE_FRAMES = "$Frames"
foreach ($name in $examples) {
  foreach ($base in $Base) {
    $exe = Join-Path "target/$profileDir/examples" "$name.exe"
    $label = "{0,-14} {1,-5}" -f $name, $base
    if (-not (Test-Path $exe)) {
        Write-Host ("  {0} MISSING  {1}" -f $label, $exe) -ForegroundColor Red
        $failed += "$name/$base"
        continue
    }
    $err = New-TemporaryFile
    $sw = [Diagnostics.Stopwatch]::StartNew()
    # stdout is nobody's: what an example prints is not what is judged.
    $p = Start-Process -FilePath $exe -ArgumentList @("--$base") -PassThru -NoNewWindow `
        -RedirectStandardError $err -RedirectStandardOutput NUL
    $done = $p.WaitForExit($TimeoutSec * 1000)
    if (-not $done) {
        try { $p.Kill($true) } catch {}
        $p.WaitForExit(5000) | Out-Null
    }
    $sw.Stop()

    $stderr = (Get-Content $err -Raw)
    if ($null -eq $stderr) { $stderr = '' }
    # kui prints one line per misconfiguration a frame noticed. They are not
    # crashes and do not fail the run, but an example of all things should
    # not be producing them, so they are reported.
    $warnings = @($stderr -split "`n" | Where-Object { $_ -match '^kui: warning' })

    if (-not $done) {
        Write-Host ("  {0} HUNG     {1}s" -f $label, $TimeoutSec) -ForegroundColor Red
        $failed += "$name/$base"
    } elseif ($p.ExitCode -ne 0) {
        Write-Host ("  {0} FAILED   exit {1}" -f $label, $p.ExitCode) -ForegroundColor Red
        ($stderr -split "`n" | Select-Object -First 12) | ForEach-Object {
            if ($_.Trim()) { Write-Host "                       $_" -ForegroundColor DarkGray }
        }
        $failed += "$name/$base"
    } else {
        $note = if ($warnings.Count -gt 0) { " ($($warnings.Count) warning(s))" } else { '' }
        Write-Host ("  {0} ok       {1:N2}s{2}" -f $label, $sw.Elapsed.TotalSeconds, $note) -ForegroundColor Green
        $warnings | ForEach-Object { Write-Host "                       $_" -ForegroundColor Yellow }
    }
    Remove-Item $err -ErrorAction SilentlyContinue
  }
}
Remove-Item Env:\KUI_SMOKE_FRAMES -ErrorAction SilentlyContinue

Write-Host ""
if ($failed.Count -gt 0) {
    Write-Host ("FAILED: {0}" -f ($failed -join ', ')) -ForegroundColor Red
    exit 1
}
Write-Host ("all {0} examples drew {1} frames on each base and exited cleanly" -f $examples.Count, $Frames) -ForegroundColor Green
exit 0
