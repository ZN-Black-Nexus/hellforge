#!/usr/bin/env bash
# Play the same scripted session with every Linux build of GAME under
# qemu-user and check the final screen hashes to the same value as the
# native x86_64 build. Any endianness, word-size or code-generation bug
# shows up as a mismatch.
#   scripts/test-qemu.sh GAME ["INPUT"] [FRAMES]
set -u
cd "$(dirname "$0")/.."
. scripts/targets.sh
GAME=${1:?usage: scripts/test-qemu.sh GAME [INPUT] [FRAMES]}
INPUT=${2:-right:30 a down:20 left:15 b wait:10}
FRAMES=${3:-240}
Q=${QEMU_DIR:-$HOME/.cache/hellbyte-tools/qemu/usr/bin}
ARGS=(--frames "$FRAMES" --input "$INPUT" --hash)
ref=$("dist/$GAME-linux-x86_64" "${ARGS[@]}" | sed -nE 's/.*hash ([0-9a-f]+).*/\1/p')
echo "reference hash (x86_64 native): $ref"
pass=0; failn=0
# Oldest matching core per binary, so instructions from a newer ISA trap.
declare -A CPU=( [linux-powerpc32-spe-e500]="-cpu e500v2" [linux-armv4-strongarm-fa526]="-cpu sa1110" [linux-armv4t]="-cpu arm926" [linux-armv5te]="-cpu arm926" [linux-armv6-hf]="-cpu arm1176" [linux-armv6-sf]="-cpu arm1176" )
while IFS='|' read -r t name q extra; do
  [ -z "$t" ] && continue
  bin=dist/$GAME-$name
  [ -f "$bin" ] || continue
  if [ "$q" = "-" ]; then printf "  %-34s (no qemu for this CPU; build-only)\n" "$name"; continue; fi
  out=$(timeout 300 "$Q/qemu-$q" ${CPU[$name]:-} "$bin" "${ARGS[@]}" 2>&1)
  h=$(echo "$out" | sed -nE 's/.*hash ([0-9a-f]+).*/\1/p')
  if [ "$h" = "$ref" ]; then r="OK"; pass=$((pass+1)); else r="MISMATCH ($h) ${out:0:70}"; failn=$((failn+1)); fi
  printf "  %-34s %s\n" "$name" "$r"
done <<< "$LINUX_TARGETS"
echo "passed $pass, failed $failn"
