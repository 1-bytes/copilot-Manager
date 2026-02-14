#!/usr/bin/env bash
#
# release.sh — 一键发布 Copilot Manager 新版本
#
# 用法:
#   ./scripts/release.sh patch      # 4.1.17 → 4.1.18
#   ./scripts/release.sh minor      # 4.1.17 → 4.2.0
#   ./scripts/release.sh major      # 4.1.17 → 5.0.0
#   ./scripts/release.sh 5.0.0      # 直接指定版本号
#   ./scripts/release.sh 5.0.0 -y   # 跳过确认
#
# 流程:
#   1. 校验工作区干净
#   2. 同步三处版本号 (package.json, Cargo.toml, tauri.conf.json)
#   3. cargo check + tsc --noEmit 编译检查
#   4. git commit + tag + push → 触发 GitHub Actions CI
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── 颜色 ─────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
die()   { error "$@"; exit 1; }

# ── 版本号工具 ────────────────────────────────────────────
get_current_version() {
    grep '"version"' "$PROJECT_ROOT/package.json" | head -1 | sed 's/.*"\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)".*/\1/'
}

bump_version() {
    local current="$1" type="$2"
    local major minor patch
    IFS='.' read -r major minor patch <<< "$current"

    case "$type" in
        major) echo "$((major + 1)).0.0" ;;
        minor) echo "${major}.$((minor + 1)).0" ;;
        patch) echo "${major}.${minor}.$((patch + 1))" ;;
        *)
            # 直接用作版本号
            if [[ "$type" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                echo "$type"
            else
                die "无效的版本参数: $type (使用 major/minor/patch 或 X.Y.Z)"
            fi
            ;;
    esac
}

# ── 前置检查 ──────────────────────────────────────────────
preflight() {
    info "前置检查..."

    # 必须在项目根目录
    [[ -f "$PROJECT_ROOT/package.json" ]] || die "找不到 package.json，请在项目根目录运行"
    [[ -f "$PROJECT_ROOT/src-tauri/Cargo.toml" ]] || die "找不到 src-tauri/Cargo.toml"
    [[ -f "$PROJECT_ROOT/src-tauri/tauri.conf.json" ]] || die "找不到 src-tauri/tauri.conf.json"

    # 工具检查
    command -v git  >/dev/null 2>&1 || die "需要 git"
    command -v jq   >/dev/null 2>&1 || die "需要 jq (brew install jq)"
    command -v sed  >/dev/null 2>&1 || die "需要 sed"
    command -v npx  >/dev/null 2>&1 || die "需要 npx (Node.js)"

    # Git 状态检查
    cd "$PROJECT_ROOT"

    if ! git diff --quiet HEAD 2>/dev/null; then
        warn "工作区有未提交的改动:"
        git status --short
        echo ""
        read -rp "是否继续？改动将一起提交 (y/N): " ans
        [[ "$ans" =~ ^[Yy]$ ]] || die "取消发布"
    fi

    # remote 检查
    if ! git remote get-url origin >/dev/null 2>&1; then
        die "没有配置 git remote origin"
    fi

    ok "前置检查通过"
}

# ── 版本写入 ──────────────────────────────────────────────
write_versions() {
    local new_version="$1"
    info "写入版本号 → ${BOLD}${new_version}${NC}"

    cd "$PROJECT_ROOT"

    # 1. package.json
    local tmp
    tmp=$(jq --arg v "$new_version" '.version = $v' package.json)
    echo "$tmp" > package.json
    ok "  package.json"

    # 2. Cargo.toml — 只替换 [package] 段下的 version
    sed -i.bak -E "s/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"/version = \"${new_version}\"/" \
        src-tauri/Cargo.toml
    rm -f src-tauri/Cargo.toml.bak
    ok "  src-tauri/Cargo.toml"

    # 3. tauri.conf.json
    tmp=$(jq --arg v "$new_version" '.version = $v' src-tauri/tauri.conf.json)
    echo "$tmp" > src-tauri/tauri.conf.json
    ok "  src-tauri/tauri.conf.json"

    # 4. 更新 Cargo.lock
    cd src-tauri
    cargo update --workspace --quiet 2>/dev/null || true
    cd "$PROJECT_ROOT"
    ok "  Cargo.lock (updated)"
}

# ── 编译检查 ──────────────────────────────────────────────
compile_check() {
    info "编译检查..."

    cd "$PROJECT_ROOT/src-tauri"
    info "  cargo check..."
    if ! cargo check --quiet 2>&1; then
        die "Rust 编译失败"
    fi
    ok "  Rust ✓"

    cd "$PROJECT_ROOT"
    info "  tsc --noEmit..."
    if ! npx tsc --noEmit 2>&1; then
        die "TypeScript 编译失败"
    fi
    ok "  TypeScript ✓"
}

# ── Git 提交 + Tag + Push ─────────────────────────────────
git_release() {
    local new_version="$1"
    local tag="v${new_version}"

    cd "$PROJECT_ROOT"

    info "Git 提交..."

    # 检查 tag 是否已存在
    if git rev-parse "$tag" >/dev/null 2>&1; then
        die "Tag $tag 已存在，请先删除或使用其他版本号"
    fi

    # Stage 版本文件
    git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
    # 如果 Cargo.lock 有变化也加上
    git add src-tauri/Cargo.lock 2>/dev/null || true
    # 加上其他暂存区的改动
    git add -u

    git commit -m "chore: release ${tag}" --allow-empty

    info "创建 tag: ${BOLD}${tag}${NC}"
    git tag -a "$tag" -m "Release ${tag}"

    info "推送到 origin..."
    git push origin HEAD
    git push origin "$tag"

    ok "推送完成"
}

# ── 主函数 ────────────────────────────────────────────────
main() {
    if [[ $# -lt 1 ]]; then
        echo -e "${BOLD}用法:${NC} $0 <major|minor|patch|X.Y.Z> [-y]"
        echo ""
        echo "  patch    增加补丁版本 (z+1)"
        echo "  minor    增加次版本 (y+1, z=0)"
        echo "  major    增加主版本 (x+1, y=0, z=0)"
        echo "  X.Y.Z    直接指定版本号"
        echo "  -y       跳过确认"
        echo ""
        echo "当前版本: $(get_current_version)"
        exit 0
    fi

    local bump_type="$1"
    local skip_confirm="${2:-}"
    local current_version new_version

    current_version=$(get_current_version)
    new_version=$(bump_version "$current_version" "$bump_type")

    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║${NC}  ${BOLD}Copilot Manager — 发布新版本${NC}             ${CYAN}║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════╣${NC}"
    echo -e "${CYAN}║${NC}  当前版本:  ${YELLOW}${current_version}${NC}"
    echo -e "${CYAN}║${NC}  新版本:    ${GREEN}${BOLD}${new_version}${NC}"
    echo -e "${CYAN}║${NC}  Tag:       ${GREEN}v${new_version}${NC}"
    echo -e "${CYAN}║${NC}  仓库:      $(git remote get-url origin)"
    echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
    echo ""

    if [[ "$skip_confirm" != "-y" ]]; then
        echo -e "发布流程: 版本写入 → 编译检查 → git commit → git tag → git push → CI 自动构建"
        echo ""
        read -rp "确认发布？(y/N): " confirm
        [[ "$confirm" =~ ^[Yy]$ ]] || { info "取消发布"; exit 0; }
    fi

    echo ""

    # Step 1: 前置检查
    preflight

    # Step 2: 写入版本号
    write_versions "$new_version"

    # Step 3: 编译检查
    compile_check

    # Step 4: Git 提交 + Tag + Push
    git_release "$new_version"

    echo ""
    echo -e "${GREEN}╔══════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║${NC}  ${BOLD}发布成功!${NC}                                ${GREEN}║${NC}"
    echo -e "${GREEN}╠══════════════════════════════════════════╣${NC}"
    echo -e "${GREEN}║${NC}  版本: ${BOLD}v${new_version}${NC}"
    echo -e "${GREEN}║${NC}"
    echo -e "${GREEN}║${NC}  CI 将自动:"
    echo -e "${GREEN}║${NC}    1. 构建全平台桌面安装包"
    echo -e "${GREEN}║${NC}    2. 创建 GitHub Release"
    echo -e "${GREEN}║${NC}    3. 构建并推送 Docker 镜像到 GHCR"
    echo -e "${GREEN}║${NC}"
    echo -e "${GREEN}║${NC}  查看构建进度:"
    echo -e "${GREEN}║${NC}    $(git remote get-url origin | sed 's/\.git$//')/actions"
    echo -e "${GREEN}╚══════════════════════════════════════════╝${NC}"
    echo ""
}

main "$@"
