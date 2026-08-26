#!/usr/bin/env bash
# 构建发布包:有证书走正式签名,无证书走 ad-hoc(本地分发/测试用)。
#
# 用法:
#   ./scripts/build-release.sh              # ad-hoc 签名(keychain 弹窗问题依旧,仅供本机测试)
#   SIGNING_IDENTITY="Developer ID Application: ..." ./scripts/build-release.sh
#                                           # 正式签名(需先在钥匙串导入证书)
#   另需公证时 export APPLE_ID / APPLE_PASSWORD / APPLE_TEAM(见 tauri 文档 notarize 配置)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")
echo "==> Conduit v${VERSION} 构建开始"

if [[ -n "${SIGNING_IDENTITY:-}" ]]; then
  echo "==> 正式签名: ${SIGNING_IDENTITY}"
  export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-}"
  pnpm tauri build
else
  echo "==> 未设置 SIGNING_IDENTITY,使用 ad-hoc 签名(仅本机测试)"
  pnpm tauri build
fi

echo "==> 产物:"
ls -lh src-tauri/target/release/bundle/macos/*.app 2>/dev/null || true
ls -lh src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null || true
