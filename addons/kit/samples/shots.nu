#!/usr/bin/env nu
# Draws the samples' pictures into each sample's shots/ folder, from each
# sample's test/shot.lua and the list in its test/shots.nuon:
#
#   nu addons/kit/samples/shots.nu              every sample
#   nu addons/kit/samples/shots.nu relay        one
#   nu addons/kit/samples/shots.nu relay --out /tmp/x   somewhere else
#
# A shots.nuon is a list of { name, size, after, args }: `args` follow `--`
# to the shot script (a skin, a state).

def main [
  ...samples: string   # the samples to draw; every one when left out
  --kui: string = ""   # the kui binary; target/debug/kui when left out
  --out: string = ""   # where the pictures go; each sample's shots/ when left out
] {
  let here = $env.FILE_PWD
  let root = $here | path join ".." ".." ".." | path expand
  let bin = if $kui == "" { $root | path join "target" "debug" "kui" } else { $kui }
  let names = if ($samples | is-empty) {
    ls $here | where type == dir | get name | path basename | where { |n| ($here | path join $n "test" "shots.nuon" | path exists) }
  } else { $samples }
  for name in $names {
    let dir = $here | path join $name
    let dest = if $out == "" { $dir | path join "shots" } else { $out }
    mkdir $dest
    for s in (open ($dir | path join "test" "shots.nuon")) {
      let png = $dest | path join $"($s.name).png"
      let r = (do { ^$bin run ($dir | path join "test" "shot.lua") --shot $png --after ($s.after | into string) --size $s.size -- ...$s.args } | complete)
      let said = ($r.stdout + $r.stderr) | lines | where { |l| not ($l | str starts-with "kui: wrote") }
      if $r.exit_code != 0 or ($said | is-not-empty) {
        print $"($name)/($s.name): exit ($r.exit_code)"
        $said | each { |l| print $"  ($l)" }
      } else {
        print $"($name)/($s.name).png"
      }
    }
  }
}
