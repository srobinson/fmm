#!/usr/bin/env bash
set -euo pipefail

publish() {
  local dir=$1
  local name version

  name=$(node -p "require('./$dir/package.json').name")
  version=$(node -p "require('./$dir/package.json').version")

  if npm view "$name@$version" version 2>/dev/null; then
    echo "::notice::$name@$version already published, skipping"
  else
    local publish_args=(--access public)
    if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
      publish_args+=(--provenance)
    fi
    (cd "$dir" && npm publish "${publish_args[@]}")
  fi
}

for dir in "$@"; do
  publish "$dir"
done
