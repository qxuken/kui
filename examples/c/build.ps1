# The Windows half of build.sh: kui_ffi and the C examples against it,
# after checking that include/kui.h still describes the structs Rust
# actually lays out.
#
#   pwsh examples/c/build.ps1
#   pwsh examples/c/build.ps1 -Run       # and run the round it prints
#   pwsh examples/c/build.ps1 -Release
#
# Everything lands in target/<profile>/, beside the kui_ffi.dll the hosts
# load and the c_panel.exe a plugin can import from - so nothing is copied
# anywhere and nothing is written into the source tree:
#
#   counter.exe       C as the host, kui as a plain library
#   host.exe          C as the host of a C *extension* (ADR 0014's C half)
#   panel.dll         the panel, importing from kui_ffi.dll
#   panel-host.dll    the same panel, importing from the Rust host
#   panel-noabi.dll   the panel with kui_ext_abi deleted; must be refused
#
# What differs from build.sh is Windows', not kui's, and is worth reading:
#
#  * The compiler must be MSVC-ABI, because that is the ABI the Rust
#    x86_64-pc-windows-msvc target links with. `cl` or `clang-cl`, whichever
#    is found; a MinGW `gcc` would build and then not link. `cl` is not on
#    PATH by default, so vswhere is asked for the VS install and its
#    environment is imported once, here.
#
#  * The hosts link kui_ffi.dll through its import library
#    (kui_ffi.dll.lib) and need the DLL beside them at run time, since
#    Windows has no rpath. Building into target/<profile>/ is what puts it
#    there; it used to be a copy into examples/c/.
#
#  * A plugin links against an import library rather than against nothing. A
#    DLL may not have an unresolved import: it names the module each kui_*
#    comes from in its own import table and takes that name from a .lib.
#    Which .lib is the choice, and both are built here because both are
#    real - from one compile of panel.c, since the two differ only in what
#    the link resolves against:
#
#      panel.dll takes kui_ffi.dll's, and so loads into any host shipping
#      that DLL - the shape a plugin you hand to somebody wants. It is the
#      default for both hosts here, and the one a plugin ships in.
#
#      panel-host.dll takes the *Rust host's*, which crates/kui-ffi/build.rs
#      arranges by handing link.exe a /DEF: naming all 135 kui_* so that
#      link.exe writes c_panel.lib. That plugin loads into c_panel.exe and
#      no other, because an import library names the module it imports from.
#
#    On ELF neither choice exists or is needed: the plugin leaves them
#    undefined and the host is linked --export-dynamic, so build.sh builds
#    one panel.so and both hosts load it.
#
#  * The plugin's own seven entry points need __declspec(dllexport), which
#    kui.h puts on their declarations as KUI_EXT_EXPORT, so panel.c is the
#    same source on every platform and this script needs no export list.
#
# Exit codes: 0 everything built (and, with -Run, the round passed),
# non-zero at the first failure.

[CmdletBinding()]
param(
    # Build and link the release profile instead of dev.
    [switch]$Release,
    # Run the round afterwards instead of only printing it. This is what CI
    # runs, so that the commands a reader is given are the commands that
    # are checked.
    [switch]$Run
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..' '..')

$cargoProfile = if ($Release) { 'release' } else { 'dev' }
# Where cargo puts the `dev` profile is `debug`; the one mapping.
$profileDir = if ($Release) { 'release' } else { 'debug' }
# Everything built here, beside kui_ffi.dll; and where cargo puts an example.
$bin = "target/$profileDir"
$ex = "$bin/examples"

# --- a compiler -------------------------------------------------------------

function Import-VsDevEnv {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $false }
    $root = & $vswhere -latest -products * -property installationPath
    if (-not $root) { return $false }
    $devcmd = Join-Path $root 'Common7\Tools\VsDevCmd.bat'
    if (-not (Test-Path $devcmd)) { return $false }
    # Run the batch file and copy back the environment it set. `-arch=x64`
    # rather than the default x86: the Rust target is 64-bit and a 32-bit cl
    # would produce objects link.exe refuses.
    & cmd.exe /s /c "`"$devcmd`" -arch=x64 -no_logo && set" | ForEach-Object {
        if ($_ -match '^([^=]+)=(.*)$') { Set-Item -Path "Env:\$($Matches[1])" -Value $Matches[2] }
    }
    return $true
}

function Find-Cc {
    foreach ($c in 'cl', 'clang-cl') {
        if (Get-Command $c -ErrorAction SilentlyContinue) { return $c }
    }
}

$cc = Find-Cc
if (-not $cc -and (Import-VsDevEnv)) { $cc = Find-Cc }
if (-not $cc) {
    throw "no MSVC-ABI C compiler: install the VS Build Tools (cl) or LLVM (clang-cl). A MinGW gcc will not do - it links a different ABI than the Rust msvc target."
}
Write-Host "compiler: $cc" -ForegroundColor Cyan

# Shared flags. `/D_CRT_SECURE_NO_WARNINGS` because counter.c reads a corpus
# file with fopen/sscanf, which the CRT deprecates and no other platform does.
# `/utf-8` because the sources are UTF-8 and carry a few characters that say
# so (panel.c's " · %d left"), and cl reads a BOM-less file in the machine's
# ANSI code page unless told - which turns that literal into mojibake on
# Windows and nowhere else. gcc and clang assume UTF-8 already, so this is
# the flag that makes build.sh and this script compile the same bytes.
# `KUI_PROFILE_DIR` is what host.c builds its default plugin path out of, so
# a -Release host looks beside itself rather than in target/debug.
$cflags = @(
    '/nologo', '/utf-8', '/D_CRT_SECURE_NO_WARNINGS', "/DKUI_PROFILE_DIR=$profileDir",
    '/I', 'crates/kui-ffi/include', '/std:c11', '/W3'
)

function Invoke-Cc {
    param([string[]]$Arguments, [string]$What)
    & $cc @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$What failed" }
}

# One source (or one object) into one exe or DLL, against one import
# library. Objects go to target/, out of the way of the artifacts.
function Build-C {
    param([string]$Src, [string]$Out, [string]$Lib, [switch]$Dll)
    $shape = if ($Dll) { @('/LD') } else { @() }
    Invoke-Cc ($cflags + @($Src) + $shape + @("/Fe:$Out", '/Fo:target/', '/link', $Lib)) $Out
    Write-Host "built $Out" -ForegroundColor Green
}

# --- the library, and the Rust host ------------------------------------------

# One build for both: the cdylib the C hosts link, and c_panel.exe, whose
# link is what writes the import library panel-host.dll is linked against
# (see the header).
& cargo build --profile $cargoProfile -p kui-ffi --lib --example c_panel
if ($LASTEXITCODE -ne 0) { throw "cargo build -p kui-ffi failed" }

$ffiLib = "$bin/kui_ffi.dll.lib"
$hostLib = "$ex/c_panel.lib"
if (-not (Test-Path $hostLib)) {
    throw "$hostLib missing: the host exported no kui_*, so link.exe wrote no import library. crates/kui-ffi/build.rs is what asks for them."
}

# --- kui.h against the Rust layout -----------------------------------------

# The header is hand-written, so nothing in Rust makes it match the repr(C)
# structs in crates/kui-ffi/src/types.rs: a field added there but missing from
# - or misordered in - kui.h shifts every field after it, silently, at
# runtime. The same goes for the enums the API reads as indices into a list
# the core owns (KUI_ROLE_* and the rest). The test below regenerates a
# translation unit of _Static_asserts from the Rust layout and those lists
# (see mod abi_parity); compiling it against the header settles the two.
# Nothing links - the asserts are checked in the front end. This is worth its
# own run on Windows and not only on Linux: the layout being asserted is the
# MSVC one, and it is the compiler here that decides it.
$abi = 'target/kui-abi-assert.c'
Remove-Item $abi -ErrorAction SilentlyContinue
& cargo test -p kui-ffi --lib abi_parity
if ($LASTEXITCODE -ne 0) { throw "abi_parity test failed" }
if (-not (Test-Path $abi)) { throw "$abi missing: the test filter matched nothing" }

# clang-cl wants the clang spelling of a syntax-only run; cl has its own.
$synOnly = if ($cc -eq 'cl') { @('/Zs') } else { @('-fsyntax-only') }
Invoke-Cc ($cflags + $synOnly + $abi) 'kui.h ABI check'
$fields = (Select-String -Path $abi -Pattern '^KUI_FIELD').Count
$enums = (Select-String -Path $abi -Pattern '^KUI_ENUM').Count
Write-Host "kui.h matches Rust ($fields fields, $enums enum members)" -ForegroundColor Green

# --- the two hosts ----------------------------------------------------------

Build-C examples/c/counter.c "$bin/counter.exe" $ffiLib
Build-C examples/c/host.c "$bin/host.exe" $ffiLib

# --- the panel, in both shapes ----------------------------------------------

# Compiled once: the two DLLs are the same translation unit, and differ only
# in the import library the link resolves kui_* against.
Invoke-Cc ($cflags + @('/c', 'examples/c/panel.c', '/Fo:target/panel.obj')) 'panel.obj'
Build-C target/panel.obj "$bin/panel.dll" $ffiLib -Dll
Build-C target/panel.obj "$bin/panel-host.dll" $hostLib -Dll

# The same plugin with its kui_ext_abi deleted: a plugin built against a
# header from before ADR 0006 gave plugins a version, which is the one the
# host must refuse and used to load unchecked (backlog S1). Produced from
# panel.c by deleting the one line rather than kept as a second source, so the
# mutant cannot drift from the example. The round below loads it and requires
# the refusal; the check here is what makes a filter that stopped matching
# fail at the mutation instead of at the load.
$noabi = 'target/panel-noabi.c'
(Get-Content examples/c/panel.c) |
    Where-Object { $_ -notmatch '^uint32_t kui_ext_abi\(void\)' } |
    Set-Content -Path $noabi -Encoding utf8NoBOM
if (Select-String -Path $noabi -Pattern 'kui_ext_abi' -Quiet) {
    throw "panel-noabi.c still defines kui_ext_abi; the mutation missed"
}
Build-C $noabi "$bin/panel-noabi.dll" $hostLib -Dll

# --- the round --------------------------------------------------------------

# What the artifacts are for, as one list, so that -Run and the printout
# cannot disagree and CI runs exactly what a reader is told to run. Each is
# headless and self-asserting: it exits 0 having checked something, or
# non-zero having said what.
$round = @(
    @{ Exe = "$bin/counter.exe"; Args = @('--headless')
        Note = 'C as the host: the FFI self-test'
    }
    @{ Exe = "$bin/host.exe"; Args = @('--headless')
        Note = 'C on both sides: the slot filled, the click routed, the reply back'
    }
    @{ Exe = "$ex/c_panel.exe"; Args = @('--headless')
        Note = 'a Rust host with its own copy of the library and a plugin importing another: what the ABI 10 reply sink is for'
    }
    @{ Exe = "$ex/c_panel.exe"; Args = @('--headless', "$bin/panel-host.dll")
        Note = 'the other plugin shape: imports the host executable, loads into it alone'
    }
    @{ Exe = "$ex/c_panel.exe"; Args = @('--headless', "$bin/panel-noabi.dll")
        MustFail = 'plugin declares no ABI'
        Note = 'kui_ext_abi deleted: must be refused, not loaded'
    }
)

Write-Host ""
if (-not $Run) {
    Write-Host "next:" -ForegroundColor Cyan
    foreach ($step in $round) {
        Write-Host ("  ./{0} {1}" -f $step.Exe, ($step.Args -join ' '))
        Write-Host ("      # {0}" -f $step.Note) -ForegroundColor DarkGray
    }
    Write-Host "  # or pass -Run to have this script run them"
    Write-Host ""
    Write-Host "  ./$bin/counter.exe            # with a window"
    Write-Host "  ./$ex/c_panel.exe   # with a window"
    exit 0
}

foreach ($step in $round) {
    Write-Host ("+ ./{0} {1}" -f $step.Exe, ($step.Args -join ' ')) -ForegroundColor Cyan
    $out = & $step.Exe @($step.Args) 2>&1
    $out | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
    if (-not $step.MustFail) {
        if ($LASTEXITCODE -ne 0) { throw "$($step.Exe) failed: $($step.Note)" }
        continue
    }
    # The refusals: a non-zero exit is not enough on its own, since every
    # other way of failing to load has one too. The message is the check.
    if ($LASTEXITCODE -eq 0) { throw "$($step.Note): it was accepted" }
    if (-not ($out -match [regex]::Escape($step.MustFail))) {
        throw "$($step.Note): refused, but not for that reason"
    }
}

Write-Host ""
Write-Host ("the C round passed ({0} checks, {1} profile)" -f $round.Count, $profileDir) -ForegroundColor Green
