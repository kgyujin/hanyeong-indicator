#!/usr/bin/env bash
# 작성자: Git 이력 참조
# 작성목적: release 빌드로 macOS 앱 번들(dist/HanyeongIndicator.app)을 만들고 서명한다
# 작성일: 2026-11-14
# 변경사항 내역:
# - 2026-11-14 | 8단계 배포 준비 | 번들 생성, 버전 주입, 임시(ad-hoc)·Developer ID 서명
#
# 사용법:
#   scripts/bundle-macos.sh              # 이 Mac 아키텍처용, 임시 서명(내 Mac에서만 실행)
#   scripts/bundle-macos.sh --universal  # Apple Silicon + Intel 유니버설 바이너리
#   CODESIGN_IDENTITY="Developer ID Application: 이름 (팀ID)" scripts/bundle-macos.sh --universal
#                                        # 배포용 서명(hardened runtime). 공증(notarize)은 별도 단계
#
# 주의: 서명 신원이 바뀌면 macOS가 다른 앱으로 보고 손쉬운 사용 권한을 다시 요구한다.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

APP_NAME="HanyeongIndicator"
EXECUTABLE="hanyeong-indicator"
DIST="$ROOT/dist"
APP="$DIST/$APP_NAME.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
UNIVERSAL=false
[[ "${1:-}" == "--universal" ]] && UNIVERSAL=true

if $UNIVERSAL; then
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
        if ! rustup target list --installed | grep -qx "$target"; then
            echo "빌드 대상 $target 이 없습니다. 먼저 실행하세요: rustup target add $target" >&2
            exit 1
        fi
        cargo build --release --bin "$EXECUTABLE" --target "$target"
    done
    BINARY="$DIST/$EXECUTABLE-universal"
    mkdir -p "$DIST"
    lipo -create -output "$BINARY" \
        "target/aarch64-apple-darwin/release/$EXECUTABLE" \
        "target/x86_64-apple-darwin/release/$EXECUTABLE"
else
    cargo build --release --bin "$EXECUTABLE"
    BINARY="target/release/$EXECUTABLE"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/$EXECUTABLE"
sed "s/@VERSION@/$VERSION/g" assets/macos/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

IDENTITY="${CODESIGN_IDENTITY:--}"
if [[ "$IDENTITY" == "-" ]]; then
    # 임시 서명: 이 Mac에서만 실행 가능. 다른 Mac에서는 Gatekeeper가 막는다.
    codesign --force --sign - "$APP"
else
    # 배포용: 공증을 받으려면 hardened runtime과 보안 타임스탬프가 필요하다.
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"

echo "만든 앱: $APP (버전 $VERSION, 서명: $IDENTITY)"
