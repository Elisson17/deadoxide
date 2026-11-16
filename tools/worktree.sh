#!/bin/sh
# Parallel work on the direct port: one git worktree per slice, as a SIBLING of the repo.
#
#   sh tools/worktree.sh add <name> <branch> [base]   create ../zomboid-rs-<name> on a new branch (base: default main)
#   sh tools/worktree.sh rm  <name>                   remove it (refuses if it has uncommitted tracked changes)
#   sh tools/worktree.sh ls                           list worktrees
#
# Why a sibling: tools/oracle.sh takes the game folder as `..` of the repo, and oracle_hashes.rs as
# CARGO_MANIFEST_DIR/../../.. . A worktree anywhere else cannot see media/ and projectzomboid.jar.
# `decompiled/` and `profile/` are git-ignored folders of the main checkout; the worktree gets symlinks to them
# (read-only use: never write to them, never commit them).
set -e
cd "$(dirname "$0")/.."
main=$(pwd)
case "$1" in
  add)
    name=$2; branch=$3; base=${4:-main}
    [ -n "$name" ] && [ -n "$branch" ] || { echo "usage: sh tools/worktree.sh add <name> <branch> [base]"; exit 2; }
    dir="../zomboid-rs-$name"
    git worktree add -q "$dir" -b "$branch" "$base"
    # .gitignore lists `decompiled/` and `profile/` (directories); a symlink needs its own exclude entry
    exclude="$(git rev-parse --git-common-dir)/info/exclude"
    for e in /decompiled /profile; do grep -qx "$e" "$exclude" 2>/dev/null || echo "$e" >> "$exclude"; done
    ln -s "$main/decompiled" "$dir/decompiled"
    ln -s "$main/profile" "$dir/profile"
    mkdir -p "$dir/target/oracle"
    # luaexpose.txt is the measurement tools/gen_lua_expose.py reads; it is produced by `sh tools/oracle.sh luaexpose`
    [ -f target/oracle/luaexpose.txt ] && cp target/oracle/luaexpose.txt "$dir/target/oracle/"
    echo "worktree $dir on branch $branch (from $base)"
    ;;
  rm)
    name=$2
    [ -n "$name" ] || { echo "usage: sh tools/worktree.sh rm <name>"; exit 2; }
    git worktree remove "../zomboid-rs-$name"
    echo "removed ../zomboid-rs-$name (its branch is kept; delete it with git branch -d once merged)"
    ;;
  ls)
    git worktree list
    ;;
  *)
    echo "usage: sh tools/worktree.sh add|rm|ls ..."; exit 2
    ;;
esac
