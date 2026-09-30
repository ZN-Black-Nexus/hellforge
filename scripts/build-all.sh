#!/usr/bin/env bash
# Build one game for every supported OS and CPU into dist/:
#   scripts/build-all.sh GAME            e.g. scripts/build-all.sh template
#   scripts/build-all.sh GAME FILTER     only targets whose triple or name contains FILTER
# Needs: rustup nightly with rust-src (scripts/fetch-tools.sh), because most
# CPUs have no prebuilt Rust core; SPARC and C-SKY also need GNU ld.
set -u
cd "$(dirname "$0")/.."
. scripts/targets.sh
GAME=${1:?usage: scripts/build-all.sh GAME [FILTER]}
ONLY=${2:-}
[ -d "games/$GAME" ] || { echo "no games/$GAME"; exit 1; }
mkdir -p dist
LLVM_BIN=$(ls -d "$(rustc +nightly --print sysroot)"/lib/rustlib/*/bin | head -1)
STD=core,alloc,compiler_builtins
ok=0; fail=0
want() { [ -z "$ONLY" ] || [[ "$1" == *$ONLY* || "$2" == *$ONLY* ]]; }
report_fail() {
  printf "  %-34s %-30s FAILED\n" "$2" "$1"
  echo "$3" | grep -E "^error" -A8 | head -20 | sed 's/^/      /'
  fail=$((fail+1))
}

linux() { # triple name extra-rustflags
  local t=$1 name=$2 extra=$3 tdir="target/variants/$2"
  local env=(env) v4bx=0
  if [ "$extra" = "V4BX" ]; then
    # ARMv4 (no BX): build as ARMv4T with symbols, then rewrite BX -> MOV PC.
    v4bx=1; extra=""; env=(env CARGO_PROFILE_RELEASE_STRIP=false)
  fi
  if [ -n "$extra" ]; then
    # Append variant flags to the per-target flags from .cargo/config.toml.
    local base
    base=$(awk -v t="[target.$t]" '$0==t{f=1;next} f&&/^rustflags/{print;exit}' .cargo/config.toml | sed -E 's/^rustflags = \[//; s/\]$//; s/"//g; s/, / /g')
    env+=("CARGO_TARGET_$(echo "$t" | tr 'a-z.-' 'A-Z__')_RUSTFLAGS=$base $extra")
  fi
  local log out="dist/$GAME-$name"
  log=$("${env[@]}" cargo +nightly build -q --release -p "$GAME" --target "$t" --target-dir "$tdir" -Z build-std=$STD 2>&1)
  local bin="$tdir/$t/release/$GAME"
  if [ -f "$bin" ] && ! echo "$log" | grep -q "^error"; then
    if [ $v4bx = 1 ]; then
      python3 scripts/fix_v4bx.py "$bin" "$out.tmp" >/dev/null &&
      "$LLVM_BIN/llvm-strip" --strip-all -o "$out" "$out.tmp" && rm -f "$out.tmp"
    else
      cp "$bin" "$out"
    fi
    printf "  %-34s %-30s %8d bytes\n" "$name" "$t" "$(stat -c %s "$out")"
    ok=$((ok+1))
  else
    report_fail "$t" "$name" "$log"
  fi
}

echo "Linux (static, no libc):"
while IFS='|' read -r t name q extra; do
  [ -z "$t" ] && continue
  want "$t" "$name" && linux "$t" "$name" "$extra"
done <<< "$LINUX_TARGETS"
echo
echo "Windows (native Win32 .exe, no C runtime):"
for spec in "x86_64-pc-windows-msvc|windows-x86_64" "i686-pc-windows-msvc|windows-x86-32bit" "aarch64-pc-windows-msvc|windows-arm64"; do
  t=${spec%%|*}; name=${spec##*|}
  want "$t" "$name" || continue
  log=$(cargo +nightly build -q --release -p "$GAME" --target "$t" -Z build-std=$STD 2>&1)
  if [ -f "target/$t/release/$GAME.exe" ] && ! echo "$log" | grep -q "^error"; then
    cp "target/$t/release/$GAME.exe" "dist/$GAME-$name.exe"
    printf "  %-34s %-30s %8d bytes\n" "$name.exe" "$t" "$(stat -c %s "dist/$GAME-$name.exe")"; ok=$((ok+1))
  else
    report_fail "$t" "$name" "$log"
  fi
done
echo
echo "macOS: not built yet (the engine's macOS layer is still to come)."
echo
echo "built $ok, failed $fail"
