# The Windows half of build.sh: libkui_ffi and the C examples against it,
# after checking that include/kui.h still describes the structs Rust
# actually lays out.
#
#   pwsh examples/c/build.ps1
#   pwsh examples/c/build.ps1 -Release
#
# Everything build.sh builds and, because Windows has two plugin shapes
# rather than one, a second copy of the panel:
#
#   counter.exe       C as the host, kui as a plain library
#   host.exe          C as the host of a C *extension* (ADR 0014's C half)
#   panel.dll         the panel, importing from the Rust host c_panel.exe
#   panel-dll.dll     the same panel, importing from kui_ffi.dll
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
#  * The counter links kui_ffi.dll through its import library
#    (kui_ffi.dll.lib) and needs the DLL beside it at run time, since Windows
#    has no rpath. The script copies it there.
#
#  * A plugin links against an import library rather than against nothing. A
#    DLL may not have an unresolved import: it names the module each kui_*
#    comes from in its own import table and takes that name from a .lib.
#    Which .lib is the choice, and both are built here because both are
#    real. panel-dll.dll takes kui_ffi.dll's, and so loads into any host
#    shipping that DLL - the shape a plugin you hand to somebody wants, and
#    the one host.exe loads. panel.dll takes the *Rust host's*, which
#    crates/kui-ffi/build.rs arranges by handing link.exe a /DEF: naming all
#    135 kui_* so that link.exe writes c_panel.lib; that plugin loads into
#    c_panel.exe and no other, because an import library names the module it
#    imports from. On ELF neither choice exists or is needed: the plugin
#    leaves them undefined and the host is linked --export-dynamic.
#
#  * The plugin's own seven entry points need __declspec(dllexport), which
#    kui.h puts on their declarations as KUI_EXT_EXPORT, so panel.c is the
#    same source on every platform and this script needs no export list.
#
# Exit codes: 0 everything built, non-zero at the first failure.

[CmdletBinding()]
param(
    # Build and link the release profile instead of dev.
    [switch]$Release
)

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..' '..')

$profile = if ($Release) { 'release' } else { 'dev' }
# Where cargo puts the `dev` profile is `debug`; the one mapping.
$profileDir = if ($Release) { 'release' } else { 'debug' }

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
$cflags = @(
    '/nologo', '/D_CRT_SECURE_NO_WARNINGS',
    '/I', 'crates/kui-ffi/include', '/std:c11', '/W3'
)

function Invoke-Cc {
    param([string[]]$Arguments, [string]$What)
    & $cc @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$What failed" }
}

# One C source into one exe or DLL, linked against one import library. The
# object goes to target/ under the source's own name.
function Build-C {
    param([string]$Src, [string]$Out, [string]$Lib, [switch]$Dll)
    $shape = if ($Dll) { @('/LD') } else { @() }
    Invoke-Cc ($cflags + @($Src) + $shape + @("/Fe:$Out", '/Fo:target/', '/link', $Lib)) $Out
    Write-Host "built $Out" -ForegroundColor Green
}

# --- the library, and the Rust host ------------------------------------------

# One build for both: the cdylib the C hosts link, and c_panel.exe, whose
# link is what writes the import library panel.dll is linked against
# (see the header).
& cargo build --profile $profile -p kui-ffi --lib --example c_panel
if ($LASTEXITCODE -ne 0) { throw "cargo build -p kui-ffi failed" }

$ffiLib = "target/$profileDir/kui_ffi.dll.lib"
$hostLib = "target/$profileDir/examples/c_panel.lib"
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

# --- the counter: C as the host --------------------------------------------

Build-C examples/c/counter.c examples/c/counter.exe $ffiLib
# No rpath on Windows: the loader looks beside the exe, then on PATH.
Copy-Item "target/$profileDir/kui_ffi.dll" examples/c/ -Force

# --- the panel: C as an extension ------------------------------------------

Build-C examples/c/panel.c examples/c/panel.dll $hostLib -Dll

# --- the host: C on both sides ---------------------------------------------

# A C host that loads the same panel, through kui_ctx_add_extension /
# kui_run_with (ADR 0014's C half, ABI 10). It links kui_ffi.dll like
# counter.exe does - a host is a host - so the plugin it loads must import
# from kui_ffi.dll too rather than from an executable. That is the *other*
# Windows plugin shape, and the one that travels: a plugin built this way
# loads into any host shipping this DLL, where panel.dll above loads into
# c_panel.exe and nothing else. Both are built here because both are real.
Build-C examples/c/host.c examples/c/host.exe $ffiLib
Build-C examples/c/panel.c examples/c/panel-dll.dll $ffiLib -Dll

# The Rust host has its own statically linked copy of the library, so loading
# panel-dll.dll into it puts *two* copies in one process - which is the case
# ABI 10's reply sink exists for, and worth having a copy of the DLL beside
# it so the check in the "next" list can be run.
Copy-Item "target/$profileDir/kui_ffi.dll" "target/$profileDir/examples/" -Force

# The same plugin with its kui_ext_abi deleted: a plugin built against a
# header from before ADR 0006 gave plugins a version, which is the one the
# host must refuse and used to load unchecked (backlog S1). Produced from
# panel.c by deleting the one line rather than kept as a second source, so the
# mutant cannot drift from the example. `c_panel --headless` loads it and
# requires the refusal; the check below is what makes a filter that stopped
# matching fail here instead of there.
$noabi = 'target/panel-noabi.c'
(Get-Content examples/c/panel.c) |
    Where-Object { $_ -notmatch '^uint32_t kui_ext_abi\(void\)' } |
    Set-Content -Path $noabi -Encoding ASCII
if (Select-String -Path $noabi -Pattern 'kui_ext_abi' -Quiet) {
    throw "panel-noabi.c still defines kui_ext_abi; the mutation missed"
}
Build-C $noabi target/panel-noabi.dll $hostLib -Dll

Write-Host ""
Write-Host "next:" -ForegroundColor Cyan
Write-Host "  ./examples/c/counter.exe --headless"
Write-Host "  ./examples/c/host.exe --headless             # C on both sides"
Write-Host "  ./examples/c/host.exe --headless examples/c/panel-dll.dll"
Write-Host "  ./examples/c/counter.exe"
Write-Host "  ./target/$profileDir/examples/c_panel.exe --headless"
Write-Host "  ./target/$profileDir/examples/c_panel.exe"
Write-Host "  ./target/$profileDir/examples/c_panel.exe --headless target/panel-noabi.dll   # must be refused"
Write-Host "  ./target/$profileDir/examples/c_panel.exe --headless examples/c/panel-dll.dll"
Write-Host "      # ^ a host with its own copy of the library and a plugin with"
Write-Host "      #   another: what ABI 10's reply sink is for"
