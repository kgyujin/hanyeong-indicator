# 1단계 기술 검증 결과

측정일: 2026-11-14 · 환경: macOS 26.5.1 (Apple Silicon), Rust 1.98.0

## 결론

| 항목 | 결과 | 상태 |
|---|---|---|
| macOS 입력 모드 감지 | 분산 알림으로 10~18ms 안에 감지 (목표 200ms) | 확인 |
| macOS 입력 소스 분류 | 2벌식 → 한, ABC → A, 일본어(히라가나) → あ | 확인 |
| macOS 손쉬운 사용 권한 확인, 마우스 폴백 | 권한 상태 확인과 마우스 좌표 조회 동작 | 확인 |
| macOS 캐럿 좌표: Claude 데스크톱 앱(Electron) | 입력란과 선택 범위는 읽히지만 화면 좌표를 못 얻음(`NoBounds`). `--electron-fix`로도 같음 → 마우스 폴백 | 확인(실패) |
| macOS 캐럿 좌표(TextEdit, Chrome, VS Code 등) | 측정 못 함 | **미검증** |
| Windows 코드 | x64·ARM64 컴파일 검사와 clippy 통과 | 컴파일만 확인 |
| Windows 입력 모드·캐럿 동작 | Windows 기기 없음 | **미검증** |
| 자원 사용량(macOS 프로브, release) | 바이너리 519KB, 10초 동안 CPU 0.03초(약 0.3%), 최대 RSS 24.7MB | 확인 |

## macOS 자동 측정 (`hanyeong-probe --self-test`)

프로브가 입력 소스를 직접 바꾸고, 바꾼 순간부터 걸린 시간을 쟀다.
`notify`는 분산 알림이 도착할 때까지, `visible`은 TIS 조회 결과에 반영될 때까지의 시간이다.

| 선택한 입력 소스 | notify | visible |
|---|---|---|
| com.apple.inputmethod.Korean.2SetKorean | 12.9 ms | 13.2 ms |
| com.apple.keylayout.ABC | 15.2 ms | 19.3 ms |
| com.apple.inputmethod.Korean.2SetKorean | 17.4 ms | 18.5 ms |
| com.apple.keylayout.ABC | 18.0 ms | 19.0 ms |
| com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese | 11.9 ms | 14.3 ms |
| com.apple.keylayout.ABC (원래 소스로 복구) | 10.4 ms | 3.7 ms |

한계: 프로그램으로 소스를 바꾼 경우를 쟀다. Caps Lock이나 Ctrl+Space로 직접 바꿀 때도 같은 알림이 오는지는 아래 수동 확인 항목에 넣었다.

두 번째 측정(라벨 변경 후)에서 발견한 점:
- ABC로 한 번 바꿨는데 같은 알림이 32ms 안에 3번 왔다. 앱 본체는 알림을 받으면 이전 모드와 비교해 바뀐 경우에만 배지를 강조해야 한다.
- 이미 선택된 소스를 다시 선택하면 알림이 오지 않는다(정상).

## 배지 표기 (사용자 결정, 2026-11-14)

| 입력 상태 | 배지 | macOS 판정 근거 | Windows 판정 근거 |
|---|---|---|---|
| 한글 | 한 | 언어 `ko` | LANGID 0x12, 변환 모드 NATIVE 비트 |
| 영문, CJK 입력기 안의 영문 모드 | A | 라틴 문자권 언어, ID가 `Roman`으로 끝남 | IME 닫힘 또는 NATIVE 비트 없음 |
| 히라가나 | あ | 언어 `ja` | LANGID 0x11, NATIVE |
| 가타카나 | ア | ID가 `.Katakana`로 끝남 | NATIVE + KATAKANA + FULLSHAPE |
| 반각 가타카나 | ｱ | ID가 `.HalfWidthKana`로 끝남 | NATIVE + KATAKANA |
| 중국어 간체 | 简 | 언어 `zh-Hans` (태그에 문자 체계가 없으면 `.TCIM.`이 아닌 ID) | LANGID 0x0804, 0x1004 + NATIVE |
| 중국어 번체 | 繁 | 언어 `zh-Hant`, `yue-Hant` 또는 ID에 `.TCIM.` | LANGID 0x0404, 0x0C04, 0x1404 + NATIVE |
| 그 밖의 언어 | 언어 코드(예: RU) | 언어 코드 | LANGID 16진수 |
| 읽기 실패 | ? | 입력 소스 조회 실패 | 변환 모드 조회 실패(관리자 권한 창 등) |

알려진 한계:
- macOS 중국어 병음 입력기 안에서 Shift로 바꾸는 영문 모드는 입력 소스 ID가 바뀌지 않아 TIS로 구분할 수 없다. 이때도 简으로 표시된다. Caps Lock으로 ABC 입력 소스로 바꾸는 경우는 A로 구분된다.
- 이 Mac에는 일본어 히라가나만 켜져 있어 가타카나·반각·중국어는 단위 테스트로만 확인했다. 입력 소스 ID 규칙은 실제 ID로 확인해야 한다(**미검증**).
- Windows 일본어·중국어 IME의 변환 모드 값은 문서 기준이며 실측하지 않았다(**미검증**).

## 남은 수동 확인

### macOS (화면 잠금 해제 후)

```bash
cd ~/workspace/hanyeong-indicator
cargo run --release --bin hanyeong-probe
```

각 앱의 입력란을 클릭하고 몇 글자 입력한 뒤 결과를 적는다.

| 앱 | Caps Lock 전환이 [notify]로 잡힘 | [caret] 출력 | 비고 |
|---|---|---|---|
| TextEdit | | | |
| 메모 | | | |
| Safari | | | |
| Chrome | | | |
| VS Code | | | `--electron-fix`로도 시험 |
| 터미널 / iTerm2 | | | |
| 카카오톡 | | | |
| Slack | | | `--electron-fix`로도 시험 |

참고: 이 프로브는 실행한 터미널 앱의 손쉬운 사용 권한을 이어받는다. `[init] 손쉬운 사용 권한: 없음`이 나오면 `--prompt`를 붙여 실행한다.

### Windows

Windows 기기에서 저장소를 받아 실행한다.

```powershell
cargo run --release --bin hanyeong-probe
```

| 앱 | 한/영 키 전환 시 [poll]의 open / conversion 값 변화 | [caret] 출처(Win32Caret / UiAutomation) |
|---|---|---|
| 메모장 | | |
| Word | | |
| Chrome / Edge | | |
| VS Code | | |
| Windows Terminal | | |
| 카카오톡 | | |
| 관리자 권한 cmd | (실패 → mode=? 기대) | |
| 일본어 IME (히라가나/가타카나/영숫자 전환) | | |
| 중국어 간체 Microsoft 병음 (Shift로 중/영 전환) | | |
| 중국어 번체 주음 | | |

가장 중요한 확인: Windows 11 새 한국어 IME에서 한/영을 바꿀 때 `open`과 `conversion` 중 어느 값이 바뀌는지.
현재 분류 규칙(`src/core/classify.rs`의 `classify_windows`)은 두 값을 함께 본다. 실측 결과에 맞춰 조정한다.
