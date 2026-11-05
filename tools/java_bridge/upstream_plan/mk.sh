#!/bin/bash
# mk.sh PR: apply PR's share of the fork's diff (spec.tsv) to the current worktree and index.
# Env: MB (merge base), FB (fork commit the spec was made from). CHECK=1: only check against the index.
set -euo pipefail
pr=$1; dir=$(dirname "$0")
awk -F'\t' -v p="$pr" '$1==p' "$dir/spec.tsv" | while IFS=$'\t' read -r id kind path hunks; do
  if [ -n "${CHECK:-}" ]; then
    case $kind in
      new) echo "new $path";;
      whole) git diff "$MB" "$FB" -- "$path" | git apply --cached --check - || echo "CONFLICT $path";;
      hunks) python3 -I "$dir/pick.py" "$path" "$hunks" | git apply --cached --check - || echo "CONFLICT $path #$hunks";;
    esac
    continue
  fi
  case $kind in
    new) git checkout "$FB" -- "$path" ;;
    whole) git diff "$MB" "$FB" -- "$path" | git apply -3 --index ;;
    hunks) python3 -I "$dir/pick.py" "$path" "$hunks" | git apply -3 --index ;;
  esac
done
