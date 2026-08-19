#!/usr/bin/env bash
# Apply the duck-sqllsp label scheme to gentleeduck/duck-sqllsp.
# Idempotent: existing labels are updated; new ones created.
#
# Usage:
#   bash .github/labels.sh
#   REPO=other/repo bash .github/labels.sh

set -euo pipefail

REPO="${REPO:-gentleeduck/duck-sqllsp}"

# label name | color (hex, no #) | description
labels=(
  # Type
  "bug 🐛|D73A4A|Something is broken"
  "feat ✨|0E8A16|New capability or surface"
  "fix 🔧|E99695|Bug fix"
  "perf ⚡|FBCA04|Speedup or memory improvement"
  "refactor ♻️|1D76DB|Code change without behaviour change"
  "docs 📚|0075CA|Docs only"
  "test ✅|C2E0C6|Test only"
  "chore 🧹|EDEDED|Routine maintenance"
  "style 🎨|F9D0C4|Formatting / whitespace"
  "build 📦|C5DEF5|Build system / packaging"
  "ci 🤖|BFD4F2|CI/CD only"
  "release 🚀|5319E7|Release / version bump"
  "revert ⏪|B60205|Revert a prior change"

  # Crate scope
  "crate: parse 🌳|0366D6|dsl-parse"
  "crate: catalog 📇|0366D6|dsl-catalog"
  "crate: knowledge 📖|0366D6|dsl-knowledge"
  "crate: resolve 🔗|0366D6|dsl-resolve"
  "crate: format 🎨|0366D6|dsl-format"
  "crate: analysis 🚨|0366D6|dsl-analysis"
  "crate: completion 💡|0366D6|dsl-completion"
  "crate: hover 🔍|0366D6|dsl-hover"
  "crate: conn 🔌|0366D6|dsl-conn"
  "crate: server 🖥|0366D6|dsl-server"
  "crate: cli 💻|0366D6|dsl-cli"

  # Area
  "area: lsp 🔌|6F42C1|LSP protocol surface"
  "area: rules 📏|6F42C1|Lint rule behaviour"
  "area: completion 💡|6F42C1|Completion contexts"
  "area: formatting 📐|6F42C1|Formatter output"
  "area: introspection 🗄|6F42C1|Live DB catalog"
  "area: dialect 🐬|6F42C1|MySQL / SQLite / MSSQL specifics"
  "area: vscode 🧩|6F42C1|VS Code extension"
  "area: editors ⌨️|6F42C1|Other editor integrations"
  "area: docs 📚|6F42C1|Documentation"
  "area: perf 📊|6F42C1|Performance"

  # Status / triage
  "status: triage 🔍|E4E669|Needs maintainer review"
  "status: blocked 🚧|B60205|Blocked on external work"
  "status: in-progress 🏗|FBCA04|Actively being worked"
  "status: stale 🥱|CCCCCC|Stale, may be auto-closed"
  "status: needs-repro 🔁|D93F0B|Cannot proceed without reproduction"
  "needs: design 💡|D4C5F9|Needs design discussion"

  # Priority
  "priority: critical 🔥|B60205|Production-impacting"
  "priority: high 🔴|D93F0B|Address soon"
  "priority: medium 🟡|FBCA04|Normal queue"
  "priority: low 🟢|0E8A16|Whenever"

  # Difficulty
  "good first issue 🌱|7057FF|Approachable for newcomers"
  "help wanted 🙋|008672|Community help welcomed"
  "hacktoberfest 🎃|FF8C00|Open for Hacktoberfest"

  # Resolution
  "wontfix 🚫|FFFFFF|Will not be fixed"
  "duplicate 👯|CFD3D7|Duplicate of another issue or PR"
  "invalid ❓|E4E669|Not actionable"
  "question 💬|D876E3|Discussion / clarification"

  # Dependencies
  "dependencies 📦|0366D6|Dependency update"
  "rust 🦀|DEA584|Rust dep"
  "javascript 🟨|F1E05A|JS / TS dep"
  "github-actions 🤖|2B7489|GH Actions dep"

  # Security
  "security 🔒|B60205|Security-sensitive"

  # Breaking
  "breaking 💥|B60205|Breaking change"
)

upsert() {
  local name="$1"; local color="$2"; local desc="$3"
  if gh label list --repo "$REPO" --limit 200 --json name -q '.[].name' | grep -Fxq "$name"; then
    gh label edit "$name" --repo "$REPO" --color "$color" --description "$desc" >/dev/null
    echo "updated  $name"
  else
    gh label create "$name" --repo "$REPO" --color "$color" --description "$desc" >/dev/null
    echo "created  $name"
  fi
}

for entry in "${labels[@]}"; do
  IFS='|' read -r name color desc <<<"$entry"
  upsert "$name" "$color" "$desc"
done

echo "done"
