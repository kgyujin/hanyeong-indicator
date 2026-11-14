# 한영 표시기 (hanyeong-indicator)

텍스트 커서 옆에 현재 입력 모드를 작은 배지로 띄우는 macOS·Windows 상주 앱입니다.
키 입력을 가로채지 않고(전역 키보드 훅 없음) 운영체제가 알려 주는 입력기 상태만 읽습니다.

| 입력 상태 | 배지 |
|---|---|
| 한글 | 한 |
| 영문 (일본어·중국어 입력기의 영문 모드 포함) | A |
| 히라가나 / 가타카나 / 반각 가타카나 | あ / ア / ｱ |
| 중국어 간체 / 번체 | 简 / 繁 |
| 그 밖의 언어 | 언어 코드 (예: RU) |
| 읽기 실패 | ? |

## 언제 보이나

기본 표시 방식은 **입력을 시작할 때 잠깐**입니다.

- 띄움: 입력란을 클릭해 커서가 들어간 순간, 입력란 안에서 한/영 등 입력 모드를 바꾼 순간
- 숨김: 타이핑을 시작하거나(캐럿이 2글자 이상 움직임) 띄운 지 2초가 지나면
- 입력란이 아니면 띄우지 않습니다.
- macOS에서 손쉬운 사용 권한이 없으면 입력란인지 알 수 없어, 입력 모드를 바꾼 순간에만 마우스 옆에 2초 띄웁니다.

메뉴의 "표시 방식 > 항상"을 고르면 이전처럼 항상 표시합니다(입력란이 아니면 마우스 옆).

배지는 macOS 기본 입력 소스 표시기처럼 **텍스트 커서 바로 아래 가운데**에, **시스템 기본 모양**(밝기 모드를 따르는 회색 배지)으로 뜹니다.
언어별 색 배지는 메뉴의 "모양 > 언어별 색"으로 고를 수 있고, 전환할 때 크게 강조하는 동작은 기본으로 꺼져 있습니다.

배지 위치는 캐럿 → 입력란 영역 왼쪽 위(캐럿 좌표를 주지 않는 앱) → 마우스 순서로 정합니다.
macOS 캐럿 조회는 범위 API(`AXBoundsForRange`)를 먼저 쓰고, 실패하면 텍스트 마커 API(`AXBoundsForTextMarkerRange`, Chromium·Firefox·WebKit)로 다시 시도합니다.
앞에 있는 앱의 포커스·커서 이동 알림을 받아 바로 갱신하고, 처음 보는 앱에는 Electron 앱이 접근성 정보를 내놓도록 `AXManualAccessibility`를 보냅니다.

## 진행 상황

| 단계 | 상태 |
|---|---|
| 1. 기술 검증 (입력 모드·캐럿 프로브) | macOS 입력 모드 확인, 캐럿·Windows는 수동 확인 남음 ([결과](docs/spike-1-results.md)) |
| 2. 공통 로직 (분류, 위치 계산, 설정) | 완료 |
| 3. macOS 배지와 메뉴바 메뉴 | 구현, 일부 확인 (아래 "알려진 문제") |
| 4. macOS 권한 안내 | 구현. 권한이 있는 상태만 확인, 권한 없는 상태는 미검증 |
| 5~6. Windows 배지·알림 영역 아이콘·UIA 캐럿 | 구현. 교차 컴파일 검사만 통과, **Windows 실행은 미검증** |
| 입력 시작 시 표시 | 구현. macOS Firefox(2초 숨김)·CotEditor(타이핑으로 숨김) 확인, Windows 미검증 |
| 기본 표시기 같은 모양·위치, 인식률·반응 속도 개선 | 구현. CotEditor 확인, Chrome·Electron 앱의 텍스트 마커 경로는 미검증 |
| 7. 자동 실행 | 구현. 등록·해제는 임시 홈 폴더 테스트로만 확인(실제 로그인 항목은 등록하지 않음) |
| 8. 서명·배포 | macOS 앱 번들 스크립트(임시 서명, 유니버설) 완료. Developer ID 서명·공증·Windows 설치 파일·CI는 예정 |

## 실행 (macOS)

```bash
cargo run --release
```

- Dock 아이콘 없이 실행되고, 메뉴바에 현재 입력 모드 글자가 나타납니다.
- 메뉴바 항목을 눌러 표시 켜기/끄기, 크기, 불투명도, 위치(캐럿 아래/오른쪽), 전환 강조를 바꿀 수 있습니다.
- 캐럿 위치를 읽으려면 **손쉬운 사용 권한**이 필요합니다. 권한이 없거나 캐럿을 못 읽는 앱에서는 배지가 마우스 포인터 옆에 뜹니다.
  - 처음 실행할 때 한 번만 시스템 안내창이 뜹니다. 그 뒤에는 메뉴 맨 위 "손쉬운 사용 권한 허용…"으로 설정 화면을 열 수 있습니다.
  - 권한을 허용하면 다시 시작하지 않아도 캐럿 추적으로 바뀝니다.
  - 개발 중 `cargo run`으로 실행하면 실행한 터미널 앱의 권한을 따릅니다.
- 설정 파일: `~/Library/Application Support/dev.hanyeong.hanyeong-indicator/config.toml`

진단 로그: `HANYEONG_DEBUG=1 cargo run`으로 실행하면 배지 기준점(캐럿/마우스)과 좌표가 바뀔 때마다 출력합니다.

### 앱 번들로 설치 (macOS)

```bash
scripts/bundle-macos.sh              # dist/HanyeongIndicator.app (이 Mac 아키텍처, 임시 서명)
scripts/bundle-macos.sh --universal  # Apple Silicon + Intel (rustup target add x86_64-apple-darwin 필요)
```

- 만든 `HanyeongIndicator.app`을 응용 프로그램 폴더로 옮겨 실행합니다. 손쉬운 사용 권한은 이 앱에 따로 허용해야 합니다.
- 임시 서명 번들은 이 Mac에서만 실행됩니다. 다른 Mac에 배포하려면 `CODESIGN_IDENTITY`로 Developer ID 서명을 하고 공증을 받아야 합니다.
- 메뉴의 "로그인할 때 자동 실행"은 지금 실행 중인 파일 경로를 `~/Library/LaunchAgents/dev.hanyeong.indicator.plist`에 등록합니다.
  `cargo run`으로 실행한 상태에서 켜면 target 폴더 경로가 등록되므로, 번들을 옮긴 뒤 번들에서 켜세요.

## 실행 (Windows)

```powershell
cargo run --release
```

- 알림 영역에 현재 모드 글자 아이콘이 나타나고, 아이콘 메뉴로 설정을 바꿉니다. 별도 권한은 필요 없습니다.
- 캐럿 위치는 Win32 시스템 캐럿(메모장 등) → UI Automation(Chrome, Edge, Office 등) → 마우스 위치 순서로 정합니다.
- 관리자 권한으로 실행한 창에서는 입력 모드를 읽을 수 없어 "?"로 표시됩니다.
- 진단 로그: `$env:HANYEONG_DEBUG=1; cargo run`(디버그 빌드는 콘솔 창이 함께 뜹니다). 입력기 원시 값(open, conversion)도 함께 출력합니다.
- 설정 파일: `%APPDATA%\hanyeong\hanyeong-indicator\config\config.toml`
- "로그인할 때 자동 실행"은 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`에 등록합니다(관리자 권한 불필요).

## 진단 도구

```bash
cargo run --release --bin hanyeong-probe -- --help
```

입력 모드와 캐럿 위치가 바뀔 때마다 원시 값을 출력합니다. Windows에서는 이 도구로 입력기 동작을 확인합니다.

## 개발

```bash
cargo test
cargo clippy --all-targets
cargo clippy --all-targets --target x86_64-pc-windows-msvc
cargo fmt --check
```

## 알려진 문제

- **메뉴바 항목이 안 보일 수 있음:** 메뉴바에 항목이 많거나 노치가 있는 화면에서는 macOS가 새 항목을 화면 밖으로 밀어냅니다.
  2026-11-14 측정에서 항목은 생성됐지만 위치가 화면 밖(x=-2201)이었습니다. 다른 메뉴바 항목을 줄이거나 시스템 설정의 메뉴 막대 항목에서 허용 여부를 확인하세요.
- **Chromium·Electron 앱(macOS):** Claude 데스크톱 앱 입력창은 캐럿 좌표 요청에 빈 사각형을 돌려줘(`EmptyBounds`) 마우스 옆에 표시됩니다.
  Chrome도 측정한 20초 동안 캐럿 기준 배치가 0건이었습니다. Chromium 전용 텍스트 마커 API(`AXSelectedTextMarkerRange`)로 해결할 수 있는지 조사가 필요합니다.
- **Firefox(macOS):** 입력란 판정과 입력란 영역은 얻지만 캐럿 글자 위치를 주지 않아, 타이핑으로 숨기지 못하고 2초가 지나야 숨깁니다.
- **타이핑 판정:** 키 입력을 직접 보지 않고 캐럿 이동으로 추정합니다. 화살표 키나 클릭으로 캐럿을 옮겨도 숨겨집니다.
- **메모리:** release 빌드 상주 메모리(RSS)가 약 51MB로 목표(30MB)보다 큽니다. 원인은 아직 조사하지 않았습니다.
- **중국어 병음의 Shift 영문 전환(macOS):** 입력 소스 ID가 바뀌지 않아 简으로 계속 표시됩니다.
