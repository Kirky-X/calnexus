#!/usr/bin/env bash
# status.sh — specmark 全局状态查询。
#
# 扫描 specmark/changes/ 和 specmark/archive/ 目录，输出活动变更与历史归档概览。
# 可被 /specmark status 子命令调用，也可独立运行。
#
# 用法：
#   status.sh [--json] [--root <project-root>]
#
# 退出码：0 正常；2 参数错误。

set -euo pipefail

SCRIPT_REAL="$(readlink -f "$0")"
SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_REAL")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

JSON=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --json) JSON=1 ;;
    --root)
      shift
      [[ $# -gt 0 ]] || { echo '[ERROR] --root 需要参数' >&2; exit 2; }
      ROOT="$(cd "$1" && pwd)"
      ;;
    -h|--help)
      echo "Usage: status.sh [--json] [--root <dir>]"
      exit 0
      ;;
    *) echo "[ERROR] 未知选项: $1" >&2; exit 2 ;;
  esac
  shift
done

CHANGES_DIR="$ROOT/specmark/changes"
ARCHIVE_DIR="$ROOT/specmark/archive"

# ---------- 辅助函数 ----------

count_tasks() {
  local tasks_file="$1"
  if [[ ! -f "$tasks_file" ]]; then
    echo "0/0"
    return
  fi
  local total completed
  total=$(grep -c '^\- \[[ xX]\]' "$tasks_file" 2>/dev/null || echo 0)
  completed=$(grep -c '^\- \[[xX]\]' "$tasks_file" 2>/dev/null || echo 0)
  echo "$completed/$total"
}

infer_phase() {
  local change_dir="$1"
  local has_proposal=0 has_design=0 has_tasks=0

  [[ -f "$change_dir/proposal.md" ]] && has_proposal=1
  [[ -f "$change_dir/design.md" ]]   && has_design=1
  [[ -f "$change_dir/tasks.md" ]]    && has_tasks=1

  if [[ $has_tasks -eq 0 ]]; then
    if [[ $has_proposal -eq 1 ]]; then
      echo "propose"
    elif [[ $has_design -eq 1 ]]; then
      echo "explore"
    else
      echo "new"
    fi
    return
  fi

  # 有 tasks.md — 检查完成状态
  local total completed
  total=$(grep -c '^\- \[[ xX]\]' "$change_dir/tasks.md" 2>/dev/null || echo 0)
  completed=$(grep -c '^\- \[[xX]\]' "$change_dir/tasks.md" 2>/dev/null || echo 0)

  if [[ $total -eq 0 ]]; then
    echo "propose"
  elif [[ $completed -ge $total && $total -gt 0 ]]; then
    echo "converge"
  elif [[ $completed -gt 0 ]]; then
    echo "apply"
  else
    echo "apply"
  fi
}

# ---------- 活动变更 ----------

declare -a active_changes=()

if [[ -d "$CHANGES_DIR" ]]; then
  for d in "$CHANGES_DIR"/*/; do
    [[ -d "$d" ]] || continue
    local_name="$(basename "$d")"
    # 跳过隐藏目录（如 .locks）
    [[ "$local_name" == .* ]] && continue
    active_changes+=("$local_name")
  done
fi

# ---------- 归档变更 ----------

declare -a archived_changes=()

if [[ -d "$ARCHIVE_DIR" ]]; then
  for d in "$ARCHIVE_DIR"/*/; do
    [[ -d "$d" ]] || continue
    local_name="$(basename "$d")"
    [[ "$local_name" == .* ]] && continue
    archived_changes+=("$local_name")
  done
fi

# ---------- 输出 ----------

if [[ $JSON -eq 1 ]]; then
  # JSON 输出
  printf '{\n  "active_changes": [\n'
  first=1
  for name in "${active_changes[@]+"${active_changes[@]}"}"; do
    change_dir="$CHANGES_DIR/$name"
    phase=""
    tasks_str=""
    has_specs=0
    spec_count=0

    phase="$(infer_phase "$change_dir")"
    tasks_str="$(count_tasks "$change_dir/tasks.md")"

    if [[ -d "$change_dir/specs" ]]; then
      spec_count=$(find "$change_dir/specs" -name "spec.md" 2>/dev/null | wc -l)
      [[ $spec_count -gt 0 ]] && has_specs=1
    fi

    [[ $first -eq 0 ]] && printf ',\n'
    first=0
    printf '    {"name": "%s", "phase": "%s", "tasks": "%s", "specs": %d}' \
      "$name" "$phase" "$tasks_str" "$spec_count"
  done
  printf '\n  ],\n  "archived_changes": [\n'

  first=1
  for entry in "${archived_changes[@]+"${archived_changes[@]}"}"; do
    archive_dir="$ARCHIVE_DIR/$entry"
    meta="$archive_dir/meta.json"
    archived_at="unknown"
    synced="false"
    commit_sha="null"

    if [[ -f "$meta" ]]; then
      archived_at=$(grep -o '"archived_at": *"[^"]*"' "$meta" 2>/dev/null | head -1 | sed 's/.*: *"//;s/"//' || echo "unknown")
      synced=$(grep -o '"synced": *[a-z]*' "$meta" 2>/dev/null | head -1 | sed 's/.*: *//' || echo "false")
      commit_sha=$(grep -o '"commit_sha": *"[^"]*"' "$meta" 2>/dev/null | head -1 | sed 's/.*: *"//;s/"//' || echo "null")
    fi

    [[ $first -eq 0 ]] && printf ',\n'
    first=0
    printf '    {"dir": "%s", "archived_at": "%s", "synced": %s, "commit_sha": "%s"}' \
      "$entry" "$archived_at" "$synced" "$commit_sha"
  done
  printf '\n  ]\n}\n'
else
  # 人类可读输出
  printf '## Specmark 状态\n\n'

  if [[ ${#active_changes[@]} -eq 0 ]]; then
    printf '**活动变更：** 无\n\n'
  else
    printf '**活动变更：** %d 个\n\n' "${#active_changes[@]}"
    printf '| 变更名 | 阶段 | 进度 | delta spec |\n'
    printf '|--------|------|------|------------|\n'
    for name in "${active_changes[@]}"; do
      change_dir="$CHANGES_DIR/$name"
      phase=""
      tasks_str=""
      spec_info=""

      phase="$(infer_phase "$change_dir")"
      tasks_str="$(count_tasks "$change_dir/tasks.md")"

      if [[ -d "$change_dir/specs" ]]; then
        sc=$(find "$change_dir/specs" -name "spec.md" 2>/dev/null | wc -l)
        if [[ $sc -gt 0 ]]; then
          spec_info="$sc 个"
        else
          spec_info="无"
        fi
      else
        spec_info="无"
      fi

      printf '| %s | %s | %s | %s |\n' "$name" "$phase" "$tasks_str" "$spec_info"
    done
    printf '\n'
  fi

  if [[ ${#archived_changes[@]} -eq 0 ]]; then
    printf '**已归档变更：** 无\n'
  else
    printf '**已归档变更：** %d 个（最近 5 个）\n\n' "${#archived_changes[@]}"
    printf '| 归档目录 | 日期 | synced | commit |\n'
    printf '|----------|------|--------|--------|\n'
    count=0
    # 倒序显示最近 5 个
    for (( i=${#archived_changes[@]}-1; i>=0 && count<5; i--, count++ )); do
      entry="${archived_changes[$i]}"
      archive_dir="$ARCHIVE_DIR/$entry"
      meta="$archive_dir/meta.json"
      archived_at="—"
      synced_str="—"
      sha_short="—"

      if [[ -f "$meta" ]]; then
        archived_at=$(grep -o '"archived_at": *"[^"]*"' "$meta" 2>/dev/null | head -1 | sed 's/.*: *"//;s/"//' || echo "—")
        synced_val=$(grep -o '"synced": *[a-z]*' "$meta" 2>/dev/null | head -1 | sed 's/.*: *//' || echo "—")
        [[ "$synced_val" == "true" ]] && synced_str="✓" || synced_str="✗"
        sha_full=$(grep -o '"commit_sha": *"[^"]*"' "$meta" 2>/dev/null | head -1 | sed 's/.*: *"//;s/"//' || echo "")
        [[ -n "$sha_full" && "$sha_full" != "null" ]] && sha_short="${sha_full:0:7}" || sha_short="null"
      fi

      printf '| %s | %s | %s | %s |\n' "$entry" "$archived_at" "$synced_str" "$sha_short"
    done
    printf '\n'
  fi
fi
