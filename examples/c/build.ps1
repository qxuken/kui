# The Windows half of build.sh: libkui_ffi and the C counter against it, then
# the C panel as a plugin for the Rust host — after checking that
# include/kui.h still describes the structs Rust actually lays out.
#
#   pwsh examples/c/build.ps1
#   pwsh examples/c/build.ps1 -Release
#
# Same three artifacts and the same checks as build.sh, in the same order.
# What differs is Windows', not kui's, and it is worth reading once:
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
#  * The plugin links against the *host executable's* import library, not
#    against nothing. A DLL may not have an unresolved import: it names the
#    module each kui_* comes from in its own import table and takes that name
#    from a .lib. crates/kui-ffi/build.rs is what makes the host export its
#    135 kui_* so link.exe writes that c_panel.lib; on ELF the plugin leaves
#    them undefined and the host only has to be linked --export-dynamic.
#
#    The consequence is Windows' too: an import library names the module it
#    imports from, so panel.dll loads into c_panel.exe and no other host,
#    where the same panel.so would have loaded into either. A host of your
#    own exports kui_* the same way and ships the .lib its plugins link to.
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

$cc = $null
foreach ($candidate in 'cl', 'clang-cl') {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) { $cc = $candidate; break }
}
if (-not $cc) {
    if (Import-VsDevEnv -and (Get-Command cl -ErrorAction SilentlyContinue)) {
        $cc = 'cl'
    } elseif (Get-Command clang-cl -ErrorAction SilentlyContinue) {
        $cc = 'clang-cl'
    }
}
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
    $output = & $cc @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        $output | ForEach-Object { Write-Host $_ -ForegroundColor DarkGray }
        throw "$What failed"
    }
}

# --- the library ------------------------------------------------------------

if ($Release) { & cargo build --release -p kui-ffi } else { & cargo build -p kui-ffi }
if ($LASTEXITCODE -ne 0) { throw "cargo build -p kui-ffi failed" }

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

Invoke-Cc ($cflags + @(
        'examples/c/counter.c',
        '/Fe:examples/c/counter.exe',
        '/Fo:target/counter.obj',
        '/link', "target/$profileDir/kui_ffi.dll.lib"
    )) 'counter'

# No rpath on Windows: the loader looks beside the exe, then on PATH.
Copy-Item "target/$profileDir/kui_ffi.dll" examples/c/ -Force
Write-Host "built examples/c/counter.exe (with kui_ffi.dll beside it)" -ForegroundColor Green

# --- the panel: C as an extension ------------------------------------------

# The host first, because linking the plugin needs the import library that
# building the host produces (see the header comment).
if ($Release) {
    & cargo build --release -p kui-ffi --example c_panel
} else {
    & cargo build -p kui-ffi --example c_panel
}
if ($LASTEXITCODE -ne 0) { throw "cargo build --example c_panel failed" }

$hostLib = "target/$profileDir/examples/c_panel.lib"
if (-not (Test-Path $hostLib)) {
    throw "$hostLib missing: the host exported no kui_*, so link.exe wrote no import library. crates/kui-ffi/build.rs is what asks for them."
}

Invoke-Cc ($cflags + @(
        'examples/c/panel.c', '/LD',
        '/Fe:examples/c/panel.dll',
        '/Fo:target/panel.obj',
        '/link', $hostLib
    )) 'panel.dll'
Write-Host "built examples/c/panel.dll" -ForegroundColor Green

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
Invoke-Cc ($cflags + @(
        $noabi, '/LD',
        '/Fe:target/panel-noabi.dll',
        '/Fo:target/panel-noabi.obj',
        '/link', $hostLib
    )) 'panel-noabi.dll'
Write-Host "built target/panel-noabi.dll (kui_ext_abi deleted; must be refused)" -ForegroundColor Green

Write-Host ""
Write-Host "next:" -ForegroundColor Cyan
Write-Host "  ./examples/c/counter.exe --headless"
Write-Host "  ./examples/c/counter.exe"
Write-Host "  cargo run -p kui-ffi --example c_panel -- --headless"
Write-Host "  cargo run -p kui-ffi --example c_panel"
