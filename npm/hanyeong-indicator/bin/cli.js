#!/usr/bin/env node
/*
 * 작성자: Git 이력 참조
 * 작성목적: `npx hanyeong-indicator` 한 줄로 한영 표시기를 설치·실행·제거하는 명령
 * 작성일: 2026-11-15
 * 변경사항 내역:
 * - 2026-11-15 | npx 설치 | install(기본)·start·uninstall 명령 추가
 *
 * 동작:
 * - 실행 파일은 OS별 선택 의존성 패키지(hanyeong-indicator-darwin / -win32-x64 / -win32-arm64)에 들어 있다.
 *   npm이 내 OS에 맞는 패키지 하나만 내려받는다.
 * - install: 앱을 고정 위치에 복사하고 실행한다. npx 캐시 경로는 버전마다 바뀌므로 그대로 실행하지 않는다.
 *     macOS   ~/Applications/HanyeongIndicator.app
 *     Windows %LOCALAPPDATA%\Programs\HanyeongIndicator\
 * - 설치 위치는 환경 변수 HANYEONG_INSTALL_DIR로 바꿀 수 있다(테스트용).
 * - 외부 패키지에 의존하지 않는다(Node.js 18 이상 기본 모듈만 사용).
 */
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const PACKAGE = require("../package.json");
const LAUNCHER_NAME = "dev.hanyeong.indicator";

const PLATFORMS = {
  darwin: { package: "hanyeong-indicator-darwin", payload: "HanyeongIndicator.app" },
  "win32-x64": { package: "hanyeong-indicator-win32-x64", payload: "hanyeong-indicator.exe" },
  "win32-arm64": { package: "hanyeong-indicator-win32-arm64", payload: "hanyeong-indicator.exe" },
};

const USAGE = `한영 표시기 ${PACKAGE.version}

사용법:
  npx hanyeong-indicator            설치(또는 업데이트)하고 실행
  npx hanyeong-indicator start      설치된 앱 실행
  npx hanyeong-indicator uninstall  앱과 로그인 시 자동 실행 등록 제거
  npx hanyeong-indicator --version  버전 출력

옵션:
  --no-launch   install 후 실행하지 않음
`;

function fail(message) {
  console.error(`오류: ${message}`);
  process.exit(1);
}

function platformKey() {
  if (process.platform === "darwin") return "darwin";
  if (process.platform === "win32") return `win32-${process.arch}`;
  return `${process.platform}-${process.arch}`;
}

function platformInfo() {
  const info = PLATFORMS[platformKey()];
  if (!info) fail(`지원하지 않는 환경입니다: ${platformKey()} (macOS, Windows x64·ARM64만 지원)`);
  return info;
}

function payloadPath(info) {
  let packageJson;
  try {
    packageJson = require.resolve(`${info.package}/package.json`);
  } catch {
    fail(
      `${info.package} 패키지를 찾지 못했습니다. npm이 선택 의존성을 건너뛰었을 수 있습니다.\n` +
        `다시 시도: npx --yes hanyeong-indicator@${PACKAGE.version}`,
    );
  }
  return path.join(path.dirname(packageJson), info.payload);
}

function installDir() {
  if (process.env.HANYEONG_INSTALL_DIR) return process.env.HANYEONG_INSTALL_DIR;
  if (process.platform === "darwin") return path.join(os.homedir(), "Applications");
  const localAppData = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
  return path.join(localAppData, "Programs", "HanyeongIndicator");
}

function installedApp() {
  return process.platform === "darwin"
    ? path.join(installDir(), "HanyeongIndicator.app")
    : path.join(installDir(), "hanyeong-indicator.exe");
}

/** 실행 중인 앱을 끈다. 실행 중이 아니어도 오류로 보지 않는다. */
function stopRunning() {
  if (process.platform === "darwin") {
    spawnSync("pkill", ["-x", "hanyeong-indicator"], { stdio: "ignore" });
  } else {
    spawnSync("taskkill", ["/IM", "hanyeong-indicator.exe", "/F"], { stdio: "ignore" });
  }
}

function start() {
  const app = installedApp();
  if (!fs.existsSync(app)) fail("설치된 앱이 없습니다. 먼저 npx hanyeong-indicator 를 실행하세요.");
  if (process.platform === "darwin") {
    const result = spawnSync("open", [app], { stdio: "inherit" });
    if (result.status !== 0) fail("앱을 실행하지 못했습니다.");
  } else {
    spawn(app, [], { detached: true, stdio: "ignore" }).unref();
  }
  console.log("한영 표시기를 실행했습니다. 메뉴바(macOS) 또는 알림 영역(Windows)에서 설정을 바꿀 수 있습니다.");
}

function install(launch) {
  const info = platformInfo();
  const source = payloadPath(info);
  if (!fs.existsSync(source)) fail(`설치 파일이 없습니다: ${source}`);

  stopRunning();
  const target = installedApp();
  fs.mkdirSync(installDir(), { recursive: true });

  if (process.platform === "darwin") {
    fs.rmSync(target, { recursive: true, force: true });
    fs.cpSync(source, target, { recursive: true, verbatimSymlinks: true });
    // npm 압축 과정에서 실행 권한이 빠져도 실행되도록 다시 준다.
    fs.chmodSync(path.join(target, "Contents", "MacOS", "hanyeong-indicator"), 0o755);
    // 인터넷에서 받은 파일 표시가 붙어 있으면 실행이 막히므로 지운다(없으면 아무 일도 하지 않음).
    spawnSync("xattr", ["-dr", "com.apple.quarantine", target], { stdio: "ignore" });
  } else {
    fs.copyFileSync(source, target);
    const probe = path.join(path.dirname(source), "hanyeong-probe.exe");
    if (fs.existsSync(probe)) fs.copyFileSync(probe, path.join(installDir(), "hanyeong-probe.exe"));
  }
  console.log(`설치했습니다: ${target}`);

  if (process.platform === "darwin") {
    console.log(
      "처음 실행하면 '손쉬운 사용' 권한을 묻습니다. 허용해야 텍스트 커서 옆에 배지가 뜹니다.\n" +
        "업데이트한 뒤에는 코드 서명이 바뀌어 권한을 다시 허용해야 할 수 있습니다.",
    );
  }
  if (launch) start();
}

function uninstall() {
  stopRunning();
  const target = installedApp();
  fs.rmSync(target, { recursive: true, force: true });
  if (process.platform === "darwin") {
    fs.rmSync(path.join(os.homedir(), "Library", "LaunchAgents", `${LAUNCHER_NAME}.plist`), {
      force: true,
    });
  } else {
    fs.rmSync(path.join(installDir(), "hanyeong-probe.exe"), { force: true });
    spawnSync(
      "reg",
      ["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", LAUNCHER_NAME, "/f"],
      { stdio: "ignore" },
    );
  }
  console.log("한영 표시기를 제거했습니다. 설정 파일은 남겨 둡니다.");
}

function main(argv) {
  const args = argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) return console.log(USAGE);
  if (args.includes("--version") || args.includes("-v")) return console.log(PACKAGE.version);
  const command = args.find((arg) => !arg.startsWith("-")) || "install";
  switch (command) {
    case "install":
      return install(!args.includes("--no-launch"));
    case "start":
      return start();
    case "uninstall":
      return uninstall();
    default:
      fail(`알 수 없는 명령: ${command}\n\n${USAGE}`);
  }
}

main(process.argv);
