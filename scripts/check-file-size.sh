#!/usr/bin/env bash
set -euo pipefail

max_lines="${MAX_FILE_LINES:-400}"
status=0

while IFS= read -r file; do
  lines="$(wc -l < "$file" | tr -d ' ')"
  if [ "$lines" -gt "$max_lines" ]; then
    echo "file-size guard failed: $file has $lines lines (max $max_lines)" >&2
    status=1
  fi
done < <(git ls-files '*.rs' '*.md' '*.toml' '*.proto' '*.yml' '*.yaml')

exit "$status"
