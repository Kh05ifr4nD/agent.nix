#!/usr/bin/env bash
set -euo pipefail

type="$1"
name="$2"
current_version="$3"

system="${SYSTEM:-x86_64-linux}"
pr_labels="${PR_LABELS:-dependencies,automated}"
auto_merge="${AUTO_MERGE:-false}"

export NIX_PATH=nixpkgs=flake:nixpkgs

if [ -z "${GH_TOKEN:-}" ]; then
  echo "Error: GH_TOKEN is not set" >&2
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "Error: working tree is not clean before update" >&2
  git status --porcelain >&2
  exit 1
fi

echo "=== Update target ==="
echo "type=$type"
echo "name=$name"
echo "system=$system"
echo "current_version=$current_version"
echo

case "$type" in
package)
  if [ -f "packages/$name/update.py" ]; then
    echo "Running packages/$name/update.py"
    packages/"$name"/update.py
  else
    echo "No update.py for $name; running nix-update"
    args=()
    if [ -f "packages/$name/nix-update-args" ]; then
      mapfile -t args <"packages/$name/nix-update-args"
    fi
    nix-update --flake "$name" "${args[@]}"
  fi
  ;;
flake-input)
  echo "Running nix flake update $name"
  nix flake update "$name"
  ;;
*)
  echo "Error: unknown type '$type' (expected 'package' or 'flake-input')" >&2
  exit 1
  ;;
esac

if git diff --quiet; then
  echo "No changes detected; skipping PR."
  exit 0
fi

echo "Regenerating README package docs (if needed)..."
./scripts/generate-package-docs.py

echo "Formatting repository..."
nix fmt

if git diff --quiet; then
  echo "No changes detected after formatting; skipping PR."
  exit 0
fi

new_version="unknown"
case "$type" in
package)
  new_version="$(nix eval --raw --impure ".#packages.${system}.\"${name}\".version" 2>/dev/null || echo "unknown")"
  ;;
flake-input)
  new_version="$(jq -r ".nodes.\"${name}\".locked.rev // \"unknown\"" flake.lock | head -c 8)"
  ;;
esac

echo "=== Validation ==="
case "$type" in
package)
  nix build --accept-flake-config --no-link ".#checks.${system}.pkgs-${name}"
  nix build --accept-flake-config --no-link ".#checks.${system}.pkgs-formatter-check"
  ;;
flake-input)
  nix flake check --no-build --accept-flake-config
  nix build --accept-flake-config --no-link ".#checks.${system}.pkgs-formatter-check"
  ;;
esac

changed_files="$(git diff --name-only)"
untracked_files="$(git ls-files --others --exclude-standard)"

all_files="$(printf "%s\n%s\n" "$changed_files" "$untracked_files" | sed '/^$/d' | sort -u)"
if [ -z "$all_files" ]; then
  echo "Error: expected changes but working tree is clean" >&2
  exit 1
fi

echo "=== Worktree changes ==="
echo "$all_files"
echo

is_allowed_change() {
  local file="$1"
  case "$type" in
  package)
    if [ "$file" = "README.md" ]; then
      return 0
    fi
    case "$file" in
    "packages/$name/"*) return 0 ;;
    esac
    return 1
    ;;
  flake-input)
    case "$file" in
    flake.lock) return 0 ;;
    README.md) return 0 ;;
    esac
    return 1
    ;;
  esac
}

while IFS= read -r file; do
  if ! is_allowed_change "$file"; then
    echo "Error: unexpected change outside allowed scope: $file" >&2
    echo "Hint: package updates must only touch packages/$name/** and optionally README.md" >&2
    echo "Hint: flake-input updates must only touch flake.lock and optionally README.md" >&2
    exit 1
  fi
done <<<"$all_files"

branch=""
pr_title=""
pr_body=""
case "$type" in
package)
  branch="update/${name}"
  pr_title="${name}: ${current_version} -> ${new_version}"
  pr_body="Automated update of ${name} from ${current_version} to ${new_version}."
  ;;
flake-input)
  branch="update/flake-input/${name}"
  pr_title="flake.lock: Update ${name}"
  pr_body="This PR updates the flake input \`${name}\`.\n\n- ${name}: \`${current_version}\` → \`${new_version}\`"
  ;;
esac

echo "=== Create/Update PR ==="
echo "branch=$branch"
echo "title=$pr_title"
echo

git switch -C "$branch"

if [ "$type" = "package" ]; then
  git add "packages/$name" README.md
else
  git add flake.lock README.md
fi

if git diff --cached --quiet; then
  echo "Error: nothing staged for commit" >&2
  exit 1
fi

git commit -m "$pr_title" --signoff

git push --force --set-upstream origin "$branch"

pr_number="$(gh pr list --head "$branch" --json number --jq '.[0].number // empty')"

label_args=()
IFS=',' read -ra labels <<<"$pr_labels"
for label in "${labels[@]}"; do
  label="$(echo "$label" | xargs)"
  [ -n "$label" ] || continue
  label_args+=(--label "$label")
done

if [ -n "$pr_number" ]; then
  echo "Updating existing PR #$pr_number"
  gh pr edit "$pr_number" --title "$pr_title" --body "$pr_body" "${label_args[@]}"
else
  echo "Creating new PR"
  gh pr create \
    --title "$pr_title" \
    --body "$pr_body" \
    --base main \
    --head "$branch" \
    "${label_args[@]}"
  pr_number="$(gh pr list --head "$branch" --json number --jq '.[0].number')"
fi

if [ "$auto_merge" = "true" ] && [ -n "$pr_number" ]; then
  echo "Enabling auto-merge for PR #$pr_number"
  gh pr merge "$pr_number" --auto --squash || echo "Note: auto-merge may require branch protection rules"
fi
