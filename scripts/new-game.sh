#!/usr/bin/env bash
# Start a new game from games/template:
#   scripts/new-game.sh NAME ["Window Title"]
# NAME becomes the crate/binary name (lowercase, digits, - and _).
set -eu
cd "$(dirname "$0")/.."
name=${1:?usage: scripts/new-game.sh NAME [TITLE]}
title=${2:-$name}
case "$name" in
  [a-z]*) ;;
  *) echo "NAME must start with a lowercase letter"; exit 1 ;;
esac
if printf '%s' "$name" | grep -q '[^a-z0-9_-]'; then
  echo "NAME may only use lowercase letters, digits, - and _"; exit 1
fi
if [ -e "games/$name" ]; then echo "games/$name already exists"; exit 1; fi
cp -R games/template "games/$name"
edit() { sed "$1" "$2" > "$2.tmp" && mv "$2.tmp" "$2"; }
edit "s/^name = \"template\"/name = \"$name\"/" "games/$name/Cargo.toml"
edit "s/^description = .*/description = \"$title, made with Hellforge\"/" "games/$name/Cargo.toml"
edit "s|^//! Template: .*|//! $title|" "games/$name/src/main.rs"
edit "s/const TITLE: \&'static str = \"Template\";/const TITLE: \&'static str = \"$title\";/" "games/$name/src/main.rs"
echo "created games/$name. Try it:"
echo "  cargo run -p $name                                   # play in a window"
echo "  cargo run -q -p $name -- --frames 60 --shot /tmp/$name.png --ascii   # look without a screen"
