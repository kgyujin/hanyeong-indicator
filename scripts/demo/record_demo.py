#!/usr/bin/env python3
"""
작성자: Git 이력 참조
작성목적: README용 데모 GIF와 스크린샷을 실제 화면에서 녹화한다(macOS 전용)
작성일: 2026-11-15
변경사항 내역:
- 2026-11-15 | README 데모 | TextEdit 입력·한영 전환 장면 녹화, GIF·PNG 변환
- 2026-11-15 | 장면 분리 | macOS 기본 표시기(파란 칩)가 찍히지 않도록 입력 소스는 녹화 밖에서 바꾸고,
  언어별 "입력란 진입 → 타이핑" 장면만 따로 찍어 이어 붙임. 한글·일본어는 키 코드로 입력해 입력기를 거치게 함

주의(실행 전 확인):
- 약 30초 동안 TextEdit를 앞으로 가져와 자동으로 타이핑한다. 그동안 키보드와 마우스를 쓰지 않는다.
- 자동 타이핑(System Events)과 화면 녹화(screencapture)에 손쉬운 사용·화면 기록 권한이 필요하다.
- 녹화 영역은 TextEdit 창 영역만이다. 같은 위치·같은 제목의 빈 TextEdit 창(가림막)을 두고
  두 창을 바꿔 "입력란 진입" 장면을 만든다. 다른 앱의 떠 있는 창이 끼어들 수 있으므로
  만든 뒤 docs/images/frames-check.png로 모든 장면을 눈으로 확인한다.
- 끝나면 원래 입력 소스로 되돌리고, 녹화용 문서 두 개만 저장하지 않고 닫는다.

필요한 것: release 빌드(cargo build --release), ffmpeg, 켜 둔 입력 소스(ABC, 한국어 2벌식, 일본어 로마자 입력)

사용법: python3 scripts/demo/record_demo.py  →  docs/images/ 에 demo.gif와 장면별 PNG 생성
"""

import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "target/release/hanyeong-indicator"
PROBE = ROOT / "target/release/hanyeong-probe"
OUT_DIR = ROOT / "docs/images"

ABC = "com.apple.keylayout.ABC"
KOREAN = "com.apple.inputmethod.Korean.2SetKorean"
JAPANESE = "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese"

# TextEdit 창 위치(주 모니터 좌상단 기준, 포인트). 녹화 영역도 이 창과 같다.
# 2026-11-15 첫 녹화에서 화면 왼쪽 위에 다른 앱의 떠 있는 창이 끼어들어 오른쪽 아래로 옮겼다.
LEFT, TOP, WIDTH, HEIGHT = 1000, 760, 560, 200
CLIP_SECONDS = 4

# 글자 크기 20pt(\fs40, 반 포인트 단위)인 빈 RTF. 끝에 이어 쓰는 글자도 이 크기를 따른다.
RTF = "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Helvetica;}}\\f0\\fs40 \\u8203?}"

# macOS ANSI 자판 키 코드. 입력기(한글·일본어)를 거치려면 글자가 아니라 키 코드를 보내야 한다.
KEY_CODES = {
    "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
    "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "o": 31, "u": 32,
    "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46, " ": 49, "\n": 36,
}

# (입력 소스, 입력할 키, 스크린샷 파일 이름)
SCENES = [
    (ABC, "hello ", "badge-latin.png"),
    (KOREAN, "dkssudgktpdy ", "badge-hangul.png"),  # 2벌식으로 "안녕하세요 "
    (JAPANESE, "konnnichiha\n", "badge-japanese.png"),  # 로마자 입력으로 "こんにちは" 후 확정
]
STILL_SECOND = 0.9


def run(*args: str, check: bool = True) -> str:
    return subprocess.run(args, check=check, capture_output=True, text=True).stdout.strip()


def osascript(script: str, check: bool = True) -> str:
    return run("osascript", "-e", script, check=check)


def press_keys(keys: str, delay: float = 0.13) -> None:
    lines = [f"key code {KEY_CODES[key]}\ndelay {delay}" for key in keys]
    osascript('tell application "System Events"\n' + "\n".join(lines) + "\nend tell")


def select_source(source_id: str) -> None:
    run(str(PROBE), "--select", source_id, check=False)


def swap_windows() -> None:
    """앞뒤 TextEdit 창을 바꾼다. 앞으로 온 창의 입력란으로 포커스가 들어간다."""
    osascript('tell application "TextEdit" to set index of window 2 to 1')


def record_clip(path: Path, keys: str) -> None:
    recorder = subprocess.Popen(
        ["screencapture", "-x", "-v", "-V", str(CLIP_SECONDS),
         "-R", f"{LEFT},{TOP},{WIDTH},{HEIGHT}", str(path)],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    start = time.monotonic()

    def at(seconds: float) -> None:
        remaining = seconds - (time.monotonic() - start)
        if remaining > 0:
            time.sleep(remaining)

    at(0.3)
    swap_windows()  # 데모 창 앞으로 → 입력란 진입 → 배지 표시
    osascript('tell application "System Events" to key code 125 using command down')  # 문서 끝으로
    at(1.9)
    press_keys(keys)  # 타이핑 시작 → 배지 숨김
    recorder.wait(timeout=CLIP_SECONDS + 10)


def main() -> int:
    if sys.platform != "darwin":
        print("macOS에서만 실행할 수 있습니다.", file=sys.stderr)
        return 1
    for tool in (APP, PROBE):
        if not tool.exists():
            print(f"{tool} 이 없습니다. 먼저 cargo build --release 를 실행하세요.", file=sys.stderr)
            return 1
    if not shutil.which("ffmpeg"):
        print("ffmpeg가 필요합니다.", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="hanyeong-demo-"))
    # 제목 표시줄이 같도록 같은 파일 이름을 다른 폴더에 둔다.
    document = work / "demo" / "Demo.rtf"
    cover = work / "cover" / "Demo.rtf"
    for path in (document, cover):
        path.parent.mkdir()
        path.write_text(RTF, encoding="ascii")
    original_source = run(str(PROBE), "--print-source", check=False)
    textedit_was_running = bool(run("pgrep", "-x", "TextEdit", check=False))

    app = subprocess.Popen([str(APP)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    clips: list[Path] = []
    try:
        time.sleep(1.0)
        bounds = f"{{{LEFT}, {TOP}, {LEFT + WIDTH}, {TOP + HEIGHT}}}"
        for path in (cover, document):
            run("open", "-a", "TextEdit", str(path))
            time.sleep(1.5)
            osascript(f'tell application "TextEdit" to set bounds of front window to {bounds}')

        for index, (source, keys, _) in enumerate(SCENES):
            # 녹화 밖: 가림막 창을 앞으로, 입력 소스 변경, 파란 칩과 배지가 사라질 때까지 대기
            swap_windows()
            select_source(source)
            time.sleep(2.8)
            clip = work / f"clip{index}.mov"
            record_clip(clip, keys)
            clips.append(clip)
    finally:
        app.terminate()
        if original_source:
            select_source(original_source)
        # 녹화용으로 연 두 문서만 닫는다(사용자가 원래 열어 둔 문서는 건드리지 않음).
        for path in (document, cover):
            osascript(
                'tell application "TextEdit"\n'
                f'close (every document whose path is "{path}") saving no\n'
                "end tell",
                check=False,
            )
        if not textedit_was_running:
            osascript('tell application "TextEdit" to quit saving no', check=False)

    missing = [clip for clip in clips if not clip.exists()]
    if len(clips) != len(SCENES) or missing:
        print("녹화 파일이 없습니다. 화면 기록 권한을 확인하세요.", file=sys.stderr)
        return 1

    # 장면 이어 붙이기 → GIF
    inputs: list[str] = []
    for clip in clips:
        inputs += ["-i", str(clip)]
    streams = "".join(f"[{i}:v]" for i in range(len(clips)))
    graph = (
        f"{streams}concat=n={len(clips)}:v=1:a=0,fps=12,scale=760:-1:flags=lanczos,"
        "split[a][b];[a]palettegen=max_colors=96[p];[b][p]paletteuse=dither=bayer"
    )
    gif = OUT_DIR / "demo.gif"
    run("ffmpeg", "-y", "-loglevel", "error", *inputs, "-filter_complex", graph, "-loop", "0", str(gif))

    # 장면별 스크린샷(배지가 보이는 시점)
    for clip, (_, _, name) in zip(clips, SCENES):
        run("ffmpeg", "-y", "-loglevel", "error", "-ss", str(STILL_SECOND), "-i", str(clip),
            "-frames:v", "1", "-vf", "scale=760:-1:flags=lanczos", str(OUT_DIR / name))

    # 확인용 장면 모음(0.5초 간격). 다른 창이 끼어들지 않았는지 눈으로 확인한 뒤 지운다.
    run("ffmpeg", "-y", "-loglevel", "error", "-i", str(gif), "-vf",
        "fps=2,scale=380:-1,tile=4x6", "-frames:v", "1", str(OUT_DIR / "frames-check.png"))

    shutil.rmtree(work, ignore_errors=True)
    print(f"만든 파일: {gif} ({gif.stat().st_size // 1024}KB)")
    for _, _, name in SCENES:
        print(f"만든 파일: {OUT_DIR / name}")
    print(f"확인용: {OUT_DIR / 'frames-check.png'} (확인 후 삭제)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
