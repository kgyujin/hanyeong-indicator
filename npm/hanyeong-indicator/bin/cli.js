#!/usr/bin/env node
/*
 * 작성자: Git 이력 참조
 * 작성목적: npx 한 줄로 한영 표시기를 설치·실행·제거하는 명령
 * 작성일: 2026-11-15
 * 변경사항 내역:
 * - 2026-11-15 | npx 설치 | install(기본)·start·uninstall 명령 추가
 * - 2026-11-15 | npm 없이 npx | OS별 npm 패키지가 없으면 GitHub 릴리스에서 실행 파일을 내려받음
 *
 * 실행 방법(둘 다 npm 레지스트리를 쓰지 않음):
 *   npx github:kgyujin/hanyeong-indicator          저장소에서 이 명령을 받음(git 필요)
 *   npx https://github.com/kgyujin/hanyeong-indicator/releases/latest/download/hanyeong-indicator.tgz
 *                                                   릴리스에 올린 묶음을 받음(git 불필요)
 *
 * 실행 파일을 찾는 순서:
 *   1. OS별 npm 패키지(hanyeong-indicator-darwin / -win32-x64 / -win32-arm64)가 설치돼 있으면 그것
 *   2. 없으면 GitHub 릴리스 v<이 패키지 버전>의 zip을 내려받아 압축을 푼 것
 *
 * install: 앱을 고정 위치에 복사하고 실행한다. npx 캐시 경로는 실행할 때마다 바뀔 수 있어 그대로 실행하지 않는다.
 *     macOS   ~/Applications/HanyeongIndicator.app
 *     Windows %LOCALAPPDATA%\Programs\HanyeongIndicator\
 * 설치 위치는 환경 변수 HANYEONG_INSTALL_DIR로 바꿀 수 있다(테스트용).
 * 외부 패키지에 의존하지 않는다(Node.js 18 이상 기본 모듈만 사용).
 */
"use strict";

const fs = require("node:fs");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const PACKAGE = require("../package.json");
const REPOSITORY = "kgyujin/hanyeong-indicator";
const LAUNCHER_NAME = "dev.hanyeong.indicator";
const DOWNLOAD_TIMEOUT_MS = 60_000;
const MAX_REDIRECTS = 5;

const PLATFORMS = {
  darwin: {
    package: "hanyeong-indicator-darwin",
    payload: "HanyeongIndicator.app",
    asset: (version) => `HanyeongIndicator-${version}-macos-universal.zip`,
  },
  "win32-x64": {
    package: "hanyeong-indicator-win32-x64",
    payload: "hanyeong-indicator.exe",
    asset: (version) => `HanyeongIndicator-${version}-windows-x64.zip`,
  },
  "win32-arm64": {
    package: "hanyeong-indicator-win32-arm64",
    payload: "hanyeong-indicator.exe",
    asset: (version) => `HanyeongIndicator-${version}-windows-arm64.zip`,
  },
};

const USAGE = `한영 표시기 ${PACKAGE.version}

사용법 (npx github:${REPOSITORY} 뒤에 붙임):
  (명령 없음)    설치(또는 업데이트)하고 실행
  start          설치된 앱 실행
  uninstall      앱과 로그인 시 자동 실행 등록 제거
  --version      버전 출력

옵션:
  --no-launch    install 후 실행하지 않음
`;

class CliError extends Error {}

function platformKey() {
  if (process.platform === "darwin") return "darwin";
  if (process.platform === "win32") return `win32-${process.arch}`;
  return `${process.platform}-${process.arch}`;
}

function platformInfo() {
  const info = PLATFORMS[platformKey()];
  if (!info) {
    throw new CliError(`지원하지 않는 환경입니다: ${platformKey()} (macOS, Windows x64·ARM64만 지원)`);
  }
  return info;
}

/** OS별 npm 패키지가 설치돼 있으면 그 안의 실행 파일 경로, 없으면 null */
function packagedPayload(info) {
  try {
    const packageJson = require.resolve(`${info.package}/package.json`);
    const payload = path.join(path.dirname(packageJson), info.payload);
    return fs.existsSync(payload) ? payload : null;
  } catch {
    return null;
  }
}

/** HTTPS로 파일을 받는다. GitHub 릴리스는 다른 주소로 넘겨주므로(302) 리디렉션을 따라간다. */
function download(url, destination, redirects = 0) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { "User-Agent": "hanyeong-indicator-installer" } }, (response) => {
      const { statusCode = 0, headers } = response;
      if (statusCode >= 300 && statusCode < 400 && headers.location) {
        response.resume();
        if (redirects >= MAX_REDIRECTS) return reject(new CliError("리디렉션이 너무 많습니다."));
        const next = new URL(headers.location, url).toString();
        return resolve(download(next, destination, redirects + 1));
      }
      if (statusCode !== 200) {
        response.resume();
        return reject(new CliError(`내려받기 실패(HTTP ${statusCode}): ${url}`));
      }
      const file = fs.createWriteStream(destination);
      response.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    });
    request.setTimeout(DOWNLOAD_TIMEOUT_MS, () => request.destroy(new CliError("내려받기 시간이 초과됐습니다.")));
    request.on("error", reject);
  });
}

function extract(zip, directory) {
  // macOS는 ditto(실행 권한·번들 구조 보존), Windows 10 1803 이상은 기본 tar가 zip을 푼다.
  const command =
    process.platform === "darwin" ? ["ditto", ["-x", "-k", zip, directory]] : ["tar", ["-xf", zip, "-C", directory]];
  const result = spawnSync(command[0], command[1], { stdio: "inherit" });
  if (result.status !== 0) throw new CliError(`압축을 풀지 못했습니다: ${zip}`);
}

/** GitHub 릴리스에서 이 패키지 버전의 실행 파일을 받아 임시 폴더에 풀고 경로를 돌려준다. */
async function downloadedPayload(info) {
  const version = PACKAGE.version;
  const asset = info.asset(version);
  const url = `https://github.com/${REPOSITORY}/releases/download/v${version}/${asset}`;
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "hanyeong-indicator-"));
  const zip = path.join(directory, asset);
  console.log(`내려받는 중: ${url}`);
  await download(url, zip);
  extract(zip, directory);
  const payload = path.join(directory, info.payload);
  if (!fs.existsSync(payload)) throw new CliError(`내려받은 파일에 ${info.payload}가 없습니다.`);
  return payload;
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
  if (!fs.existsSync(app)) throw new CliError(`설치된 앱이 없습니다. 먼저 npx github:${REPOSITORY} 를 실행하세요.`);
  if (process.platform === "darwin") {
    const result = spawnSync("open", [app], { stdio: "inherit" });
    if (result.status !== 0) throw new CliError("앱을 실행하지 못했습니다.");
  } else {
    spawn(app, [], { detached: true, stdio: "ignore" }).unref();
  }
  console.log("한영 표시기를 실행했습니다. 메뉴바(macOS) 또는 알림 영역(Windows)에서 설정을 바꿀 수 있습니다.");
}

async function install(launch) {
  const info = platformInfo();
  const source = packagedPayload(info) ?? (await downloadedPayload(info));

  stopRunning();
  const target = installedApp();
  fs.mkdirSync(installDir(), { recursive: true });

  if (process.platform === "darwin") {
    fs.rmSync(target, { recursive: true, force: true });
    fs.cpSync(source, target, { recursive: true, verbatimSymlinks: true });
    // 압축·복사 과정에서 실행 권한이 빠져도 실행되도록 다시 준다.
    fs.chmodSync(path.join(target, "Contents", "MacOS", "hanyeong-indicator"), 0o755);
    // 인터넷에서 받은 파일 표시가 붙어 있으면 실행이 막히므로 지운다(없으면 아무 일도 하지 않음).
    spawnSync("xattr", ["-dr", "com.apple.quarantine", target], { stdio: "ignore" });
  } else {
    fs.copyFileSync(source, target);
    const probe = path.join(path.dirname(source), "hanyeong-probe.exe");
    if (fs.existsSync(probe)) fs.copyFileSync(probe, path.join(installDir(), "hanyeong-probe.exe"));
  }
  console.log(`설치했습니다(${PACKAGE.version}): ${target}`);

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
  fs.rmSync(installedApp(), { recursive: true, force: true });
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

async function main(argv) {
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
      throw new CliError(`알 수 없는 명령: ${command}\n\n${USAGE}`);
  }
}

main(process.argv).catch((error) => {
  console.error(`오류: ${error instanceof CliError ? error.message : error.stack || error}`);
  process.exit(1);
});
