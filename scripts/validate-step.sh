#!/usr/bin/env bash
# ==============================================================================
# validate-step.sh — post-hoc check of one Argon step executed by another agent.
# Ported from Nebulous/scripts/validate-step.sh (2026-11-13).
#
#   scripts/validate-step.sh start <STEP_ID>     run PRE_GATE, then snapshot before handoff
#   scripts/validate-step.sh check <STEP_ID>     verify after the step was executed
#         [--no-gate]                            skip PRE_GATE / POST_GATE (scope checks only)
#
# `start` refuses to snapshot (exit 1) when the step's PRE_GATE fails, so a step
# whose spec line refs or tree assumptions went stale never reaches the executor.
#
# `check` fails (exit 1) when, compared with the `start` snapshot:
#   - HEAD moved (the executor committed),
#   - STEPPED_IMPLEMENTATION_PLAN.md changed (hashed; it is tracked here, unlike
#     Nebulous, because Step 6's acceptance test include_str!s it),
#   - any file changed, appeared or vanished outside the step's ALLOWED_FILES,
#   - the step's POST_GATE exits non-zero.
# Files already dirty at `start` count only if their content changed afterwards.
# Snapshots live in .git/validate-step/ (never tracked). Exit 2 = usage/setup error.
#
# ALLOWED_FILES entries: the first token of each "  - <path>" line. Shell glob
# patterns (*) are honoured; brace lists are not. If the ALLOWED_FILES section
# mentions Cargo.lock, Cargo.lock is allowed too.
# ==============================================================================
set -euo pipefail

usage() {
    echo "usage: $0 start|check <STEP_ID> [--no-gate]" >&2
    exit 2
}

[[ $# -ge 2 ]] || usage
MODE="$1"
STEP_ID="$2"
RUN_GATE=1
[[ "${3:-}" == "--no-gate" ]] && RUN_GATE=0
[[ "$MODE" == "start" || "$MODE" == "check" ]] || usage
[[ "$STEP_ID" =~ ^[0-9]+[A-Z]?$ ]] || { echo "STEP_ID must look like 6 or 13A" >&2; exit 2; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)" || { echo "script is not inside a git repo" >&2; exit 2; }
cd "$ROOT"
git rev-parse -q --verify HEAD >/dev/null || { echo "repo has no commits yet; make a baseline commit first" >&2; exit 2; }
PLAN="STEPPED_IMPLEMENTATION_PLAN.md"
[[ -f "$PLAN" ]] || { echo "missing $PLAN" >&2; exit 2; }
STATE_DIR="$(git rev-parse --git-dir)/validate-step"
SNAP="${STATE_DIR}/${STEP_ID}"

# --- Argon block extraction ---------------------------------------------------
BLOCK="$(awk -v id="### STEP ${STEP_ID}:" '
    index($0, id) == 1 { f = 1 }
    f { print }
    f && /^\[HALT_INVARIANT\]/ { exit }
' "$PLAN")"
[[ -n "$BLOCK" ]] || { echo "no '### STEP ${STEP_ID}:' block in $PLAN" >&2; exit 2; }

ALLOWED_SECTION="$(awk '
    /^ALLOWED_FILES:/ { f = 1; next }
    f && /^(READ_ONLY_REFS|FORBIDDEN_SCOPE):|^\[/ { exit }
    f { print }
' <<<"$BLOCK")"
mapfile -t ALLOWED < <(sed -nE 's/^[[:space:]]*-[[:space:]]+([^[:space:]]+).*/\1/p' <<<"$ALLOWED_SECTION")
[[ ${#ALLOWED[@]} -gt 0 ]] || { echo "STEP ${STEP_ID} has no ALLOWED_FILES entries" >&2; exit 2; }
grep -q 'Cargo\.lock' <<<"$ALLOWED_SECTION" && ALLOWED+=("Cargo.lock")
PRE_GATE="$(awk '/^\[PRE_GATE\]/ { getline; print; exit }' <<<"$BLOCK")"
POST_GATE="$(awk '/^\[POST_GATE\]/ { getline; print; exit }' <<<"$BLOCK")"

# --- Working-tree fingerprint: "<sha256|DELETED>  <path>" per dirty path -------
fingerprint() {
    git -c core.quotePath=false status --porcelain=v1 --no-renames --untracked-files=all |
        awk 'length($0) > 3 { print substr($0, 4) }' |
        while IFS= read -r p; do
            [[ -n "$p" ]] || continue
            if [[ -f "$p" ]]; then
                printf '%s  %s\n' "$(sha256sum -- "$p" | cut -d' ' -f1)" "$p"
            else
                printf 'DELETED  %s\n' "$p"
            fi
        done | sort -u
}

# run_gate <NAME> <COMMAND> <LOG>: returns the gate's exit status.
run_gate() {
    local name="$1" cmd="$2" log="$3" rc=0
    echo "Running ${name} (log: ${log})"
    bash -c "$cmd" >"$log" 2>&1 || rc=$?
    if [[ "$rc" -eq 0 ]]; then
        echo "${name}: exit 0"
    else
        echo "FAIL: ${name} exited ${rc}; last 30 lines:"
        tail -n 30 "$log" | sed 's/^/        /'
    fi
    return "$rc"
}

if [[ "$MODE" == "start" ]]; then
    mkdir -p "$SNAP"
    if [[ "$RUN_GATE" -eq 1 ]]; then
        [[ -n "$PRE_GATE" ]] || { echo "STEP ${STEP_ID} has no PRE_GATE line" >&2; exit 2; }
        run_gate PRE_GATE "$PRE_GATE" "${SNAP}/pre_gate.log" || {
            echo "STEP ${STEP_ID}: not handed off (PRE_GATE failed; fix the tree or the block first)"
            exit 1
        }
    else
        echo "PRE_GATE: skipped (--no-gate)"
    fi
    git rev-parse HEAD >"${SNAP}/head"
    sha256sum -- "$PLAN" | cut -d' ' -f1 >"${SNAP}/plan"
    fingerprint >"${SNAP}/tree"
    echo "STEP ${STEP_ID}: snapshot taken (HEAD $(cut -c1-12 "${SNAP}/head"), $(wc -l <"${SNAP}/tree") pre-existing dirty paths)."
    echo "Allowed files:"
    printf '  %s\n' "${ALLOWED[@]}"
    exit 0
fi

[[ -f "${SNAP}/head" ]] || { echo "no snapshot for STEP ${STEP_ID}; run: $0 start ${STEP_ID}" >&2; exit 2; }

FAIL=0
fail() { echo "FAIL: $*"; FAIL=1; }

# 1. No commits.
if [[ "$(git rev-parse HEAD)" != "$(cat "${SNAP}/head")" ]]; then
    fail "HEAD moved since start ($(cut -c1-12 "${SNAP}/head") -> $(git rev-parse --short=12 HEAD)); the executor must not commit"
fi

# 2. Plan file untouched.
if [[ "$(sha256sum -- "$PLAN" | cut -d' ' -f1)" != "$(cat "${SNAP}/plan")" ]]; then
    fail "$PLAN changed since start; the executor must not edit it"
fi

# 3. Scope: paths whose fingerprint differs from the snapshot.
is_allowed() {
    local p="$1" a
    for a in "${ALLOWED[@]}"; do
        # shellcheck disable=SC2053  # $a is intentionally a glob pattern
        [[ "$p" == $a ]] && return 0
    done
    return 1
}
mapfile -t CHANGED < <(
    comm -3 <(sort -u "${SNAP}/tree") <(fingerprint) |
        sed -E 's/^\t//; s/^[^ ]+  //' | sort -u
)
OUT_OF_SCOPE=()
for p in "${CHANGED[@]}"; do
    [[ -n "$p" ]] || continue
    [[ "$p" == "$PLAN" ]] && continue
    is_allowed "$p" || OUT_OF_SCOPE+=("$p")
done
if [[ ${#OUT_OF_SCOPE[@]} -gt 0 ]]; then
    fail "changed outside ALLOWED_FILES:"
    printf '        %s\n' "${OUT_OF_SCOPE[@]}"
fi
echo "Changed since start (${#CHANGED[@]}):"
[[ ${#CHANGED[@]} -gt 0 && -n "${CHANGED[0]}" ]] && printf '  %s\n' "${CHANGED[@]}"

# 4. POST_GATE.
if [[ "$RUN_GATE" -eq 1 ]]; then
    if [[ -z "$POST_GATE" ]]; then
        fail "STEP ${STEP_ID} has no POST_GATE line"
    else
        run_gate POST_GATE "$POST_GATE" "${SNAP}/post_gate.log" || FAIL=1
    fi
else
    echo "POST_GATE: skipped (--no-gate)"
fi

if [[ "$FAIL" -eq 0 ]]; then
    echo "STEP ${STEP_ID}: PASS"
else
    echo "STEP ${STEP_ID}: FAIL"
fi
exit "$FAIL"
