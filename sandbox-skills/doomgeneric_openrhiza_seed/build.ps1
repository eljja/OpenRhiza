param(
    [string]$Out = "..\..\rhiza_drivers\SKDOGEN.WAS",
    [switch]$EnableWasmOpt
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$doom = Join-Path $root "third_party\doomgeneric\doomgeneric"
$build = Join-Path $PSScriptRoot "build"
$clang = "C:\Program Files\LLVM\bin\clang.exe"
$wasmLd = "C:\Program Files\LLVM\bin\wasm-ld.exe"

New-Item -ItemType Directory -Force -Path $build | Out-Null

$cflags = @(
    "--target=wasm32-unknown-unknown",
    "-O3",
    "-flto",
    "-fomit-frame-pointer",
    "-ffreestanding",
    "-fno-builtin",
    "-nostdinc",
    "-DNDEBUG",
    "-DDOOMGENERIC_RESX=320",
    "-DDOOMGENERIC_RESY=200",
    "-DNORMALUNIX",
    "-DORIGINAL_DOGENERIC_OPENRHIZA",
    "-I", (Join-Path $PSScriptRoot "shim\include"),
    "-I", $doom
)

$names = @(
    "dummy","am_map","doomdef","doomstat","dstrings","d_event","d_items","d_iwad",
    "d_loop","d_main","d_mode","d_net","f_finale","f_wipe","g_game","hu_lib",
    "hu_stuff","info","i_cdmus","i_endoom","i_joystick","i_scale","i_sound",
    "i_system","i_timer","memio","m_argv","m_bbox","m_cheat","m_config",
    "m_controls","m_fixed","m_menu","m_misc","m_random","p_ceilng","p_doors",
    "p_enemy","p_floor","p_inter","p_lights","p_map","p_maputl","p_mobj",
    "p_plats","p_pspr","p_saveg","p_setup","p_sight","p_spec","p_switch",
    "p_telept","p_tick","p_user","r_bsp","r_data","r_draw","r_main","r_plane",
    "r_segs","r_sky","r_things","sha1","sounds","statdump","st_lib",
    "st_stuff","s_sound","tables","v_video","wi_stuff","w_checksum","w_file",
    "w_main","w_wad","z_zone","i_input","i_video","doomgeneric"
)

$objects = New-Object System.Collections.Generic.List[string]
foreach ($name in $names) {
    $src = Join-Path $doom "$name.c"
    $obj = Join-Path $build "$name.o"
    & $clang @cflags -c $src -o $obj
    if ($LASTEXITCODE -ne 0) { throw "clang failed for $name.c" }
    $objects.Add($obj)
}

foreach ($srcName in @("libc_shim.c", "w_file_openrhiza.c", "doomgeneric_openrhiza.c")) {
    $src = Join-Path $PSScriptRoot "src\$srcName"
    $obj = Join-Path $build ([IO.Path]::GetFileNameWithoutExtension($srcName) + ".o")
    & $clang @cflags -c $src -o $obj
    if ($LASTEXITCODE -ne 0) { throw "clang failed for $srcName" }
    $objects.Add($obj)
}

$outPath = (Join-Path $PSScriptRoot $Out)
$rawOutPath = Join-Path $build "doomgeneric.raw.wasm"
& $wasmLd `
    "--no-entry" `
    "--allow-undefined" `
    "--export=init_driver" `
    "--export=run_skill" `
    "--export=tick_skill" `
    "--export=frame_pump" `
    "--export=frame_pump_target_hz" `
    "--export=poll_net" `
    "--export=memory" `
    "--initial-memory=13631488" `
    "--max-memory=33554432" `
    "--stack-first" `
    "--gc-sections" `
    "--lto-O3" `
    @($objects.ToArray()) `
    "-o" $rawOutPath
if ($LASTEXITCODE -ne 0) { throw "wasm-ld failed" }

$wasmOpt = Join-Path $root ".tools\binaryen\node_modules\.bin\wasm-opt.cmd"
if ($EnableWasmOpt.IsPresent -and (Test-Path -LiteralPath $wasmOpt)) {
    & $wasmOpt `
        "-O2" `
        "--strip-debug" `
        "--strip-producers" `
        $rawOutPath `
        "-o" $outPath
    if ($LASTEXITCODE -ne 0) { throw "wasm-opt failed" }
} else {
    Copy-Item -LiteralPath $rawOutPath -Destination $outPath -Force
}

Write-Output "[doomgeneric] built $outPath"
