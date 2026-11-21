#!/bin/sh
# Merge Horizon's main into Skyline: `tools/sync_upstream.sh [BRANCH]`.
#
# Skyline's `multiversion` branch is a fork of a fork of Horizon. The longer it goes without
# upstream's commits the worse the merge gets (the first one, in November 2026, was 10,834
# commits and 197 conflicted files), so run this often. It:
#   1. fetches upstream's main (HORIZON_UPSTREAM_URL, default the public Horizon repository);
#   2. starts BRANCH (default `upstream-sync`) at `multiversion`, or continues it if it exists;
#   3. merges upstream's main into it, keeping Skyline's side where the policy below says so;
#   4. stops at the first conflict, listing the files, for you to resolve; and
#   5. when the merge is clean, runs the checks that have to pass before it is fast-forwarded.
# It never pushes and never changes git config. Fast-forwarding `multiversion` is yours to do:
#   git checkout multiversion && git merge --ff-only upstream-sync
#
# Policy for what the merge keeps:
#   .github/          ours (Skyline has no CI workflows; they stay deleted)
#   Cargo.lock        upstream's, then `cargo metadata` fixes it up for Skyline's crates
# Everything else is resolved by hand. docs/skyline-dev.md ("Syncing with Horizon") says what to
# look for after a merge: fragments of a patch that landed far from their place, and Skyline's
# hooks (`java_*`, `java120_*`) that upstream's restructuring moved or dropped.
set -eu

branch="${1:-upstream-sync}"
url="${HORIZON_UPSTREAM_URL:-https://github.com/horizon-rust/horizon}"
cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "sync_upstream: the working tree has uncommitted changes; commit or stash them first" >&2
    exit 1
fi

echo "== fetching $url main"
git fetch --no-tags "$url" main:refs/remotes/upstream/main

if git show-ref --verify --quiet "refs/heads/$branch"; then
    git checkout -q "$branch"
else
    git checkout -q -b "$branch" multiversion
fi

behind="$(git rev-list --count "HEAD..upstream/main")"
if [ "$behind" = 0 ]; then
    echo "== $branch already has all of upstream's main"
else
    echo "== merging $behind upstream commit(s) into $branch"
    if ! git merge --no-ff --no-commit upstream/main; then
        # Policy files first, so only real conflicts are left.
        git checkout --ours -- .github 2>/dev/null && git add .github || true
        git checkout --theirs -- Cargo.lock 2>/dev/null && git add Cargo.lock || true
        left="$(git diff --name-only --diff-filter=U)"
        if [ -n "$left" ]; then
            echo "== conflicts to resolve (then: git add FILE; re-run this script):" >&2
            echo "$left" >&2
            exit 2
        fi
    fi
    git -c core.hooksPath=.githooks commit -q --no-edit \
        -m "Merge Horizon main into Skyline ($behind commits)" \
        || { echo "sync_upstream: nothing committed; check git status" >&2; exit 1; }
fi

echo "== checks"
python3 tools/check_public_tree.py
python3 tools/java_bridge/check_no_fabric.py --tree
# The lints use the repository's pinned toolchain; -A unknown-lints keeps an older local clippy
# from stopping on a lint name that upstream's newer one defines.
for features in "--features java-mods,java-mods-120" "--features java-mods-120" "--no-default-features"; do
    echo "== clippy $features"
    # shellcheck disable=SC2086
    cargo clippy -p horizon-client --no-deps $features --all-targets -- -D warnings \
        -A unknown-lints -A clippy::manual_range_contains -A clippy::items_after_test_module
done

echo "== $branch builds and lints clean. Next:"
echo "   python3 tools/skyline_native.py -j 2     # the game, once, then try 1.20.1 and 26.3 mods"
echo "   git checkout multiversion && git merge --ff-only $branch"
