#!/usr/bin/env node
/*
 * 작성자: Git 이력 참조
 * 작성목적: 빌드 결과물로 npm 배포용 패키지 폴더(본체 1개 + OS별 실행 파일 패키지)를 만든다
 * 작성일: 2026-11-15
 * 변경사항 내역:
 * - 2026-11-15 | npx 설치 | 본체·darwin·win32-x64·win32-arm64 패키지 생성, 버전 일치, README 이미지 주소 변환
 * - 2026-11-15 | npm 없이 npx | --standalone true: OS별 패키지 의존성을 뺀 본체만 만듦(릴리스 첨부용)
 *
 * 사용법:
 *   node scripts/npm/stage.mjs --version 0.1.0 --out dist/npm \
 *     [--darwin dist/HanyeongIndicator.app] \
 *     [--win32-x64 target/x86_64-pc-windows-msvc/release] \
 *     [--win32-arm64 target/aarch64-pc-windows-msvc/release] [--standalone true]
 *
 * --standalone true: 본체 package.json에서 optionalDependencies를 뺀다. 설치 명령이 실행 파일을
 * GitHub 릴리스에서 받으므로 npm 레지스트리가 필요 없다(`npx <릴리스의 hanyeong-indicator.tgz 주소>`).
 *
 * 출력: <out>/<패키지 이름>/ 폴더. 각 폴더에서 `npm publish`하면 된다.
 * OS별 패키지를 먼저 올리고 본체를 마지막에 올려야 본체 설치 때 의존성을 찾을 수 있다.
 */
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const REPOSITORY = "kgyujin/hanyeong-indicator";
const RAW_BASE = `https://raw.githubusercontent.com/${REPOSITORY}/main/`;

const PLATFORMS = {
  darwin: {
    name: "hanyeong-indicator-darwin",
    os: ["darwin"],
    cpu: ["arm64", "x64"],
    description: "한영 표시기 macOS 앱(유니버설). hanyeong-indicator 패키지가 사용합니다.",
    copy: (source, target) => cpSync(source, join(target, "HanyeongIndicator.app"), { recursive: true }),
  },
  "win32-x64": {
    name: "hanyeong-indicator-win32-x64",
    os: ["win32"],
    cpu: ["x64"],
    description: "한영 표시기 Windows x64 실행 파일. hanyeong-indicator 패키지가 사용합니다.",
    copy: copyWindowsExecutables,
  },
  "win32-arm64": {
    name: "hanyeong-indicator-win32-arm64",
    os: ["win32"],
    cpu: ["arm64"],
    description: "한영 표시기 Windows ARM64 실행 파일. hanyeong-indicator 패키지가 사용합니다.",
    copy: copyWindowsExecutables,
  },
};

function copyWindowsExecutables(sourceDir, target) {
  for (const file of ["hanyeong-indicator.exe", "hanyeong-probe.exe"]) {
    cpSync(join(sourceDir, file), join(target, file));
  }
}

function parseArgs(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i]?.replace(/^--/, "");
    const value = argv[i + 1];
    if (!key || value === undefined) throw new Error(`잘못된 인자: ${argv.slice(i).join(" ")}`);
    options[key] = value;
  }
  if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(options.version ?? "")) {
    throw new Error("--version 1.2.3 형식의 버전이 필요합니다.");
  }
  if (!options.out) throw new Error("--out 출력 폴더가 필요합니다.");
  return options;
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

/** GitHub에서는 상대 경로가 맞지만 npm 페이지에서는 깨지므로 이미지·문서 링크를 절대 주소로 바꾼다. */
function npmReadme() {
  const readme = readFileSync(join(ROOT, "README.md"), "utf8");
  return readme
    .replace(/(\]\(|src=")(docs\/[^)"\s]+)/g, `$1${RAW_BASE}$2`)
    .replace(
      /(\]\()((?:scripts|src|assets|npm|\.github)\/[^)\s]+|LICENSE)(?=\))/g,
      `$1https://github.com/${REPOSITORY}/blob/main/$2`,
    );
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const out = resolve(options.out);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });

  const common = {
    version: options.version,
    license: "MIT",
    repository: { type: "git", url: `git+https://github.com/${REPOSITORY}.git` },
  };

  const staged = [];
  for (const [key, platform] of Object.entries(PLATFORMS)) {
    const source = options[key];
    if (!source) continue;
    if (!existsSync(source)) throw new Error(`${key} 입력이 없습니다: ${source}`);
    const target = join(out, platform.name);
    mkdirSync(target, { recursive: true });
    platform.copy(resolve(source), target);
    writeJson(join(target, "package.json"), {
      name: platform.name,
      description: platform.description,
      ...common,
      os: platform.os,
      cpu: platform.cpu,
      preferUnplugged: true,
    });
    cpSync(join(ROOT, "LICENSE"), join(target, "LICENSE"));
    staged.push(platform.name);
  }

  const main = join(out, "hanyeong-indicator");
  cpSync(join(ROOT, "npm", "hanyeong-indicator"), main, { recursive: true });
  const manifest = JSON.parse(readFileSync(join(main, "package.json"), "utf8"));
  manifest.version = options.version;
  if (options.standalone === "true") {
    delete manifest.optionalDependencies;
  } else {
    for (const name of Object.keys(manifest.optionalDependencies)) {
      manifest.optionalDependencies[name] = options.version;
    }
  }
  manifest.files = ["bin/cli.js", "README.md", "LICENSE"];
  writeJson(join(main, "package.json"), manifest);
  writeFileSync(join(main, "README.md"), npmReadme());
  cpSync(join(ROOT, "LICENSE"), join(main, "LICENSE"));
  staged.push("hanyeong-indicator");

  console.log(`npm 패키지 ${staged.length}개를 만들었습니다(${options.version}): ${out}`);
  for (const name of staged) console.log(`  ${name}`);
}

try {
  main();
} catch (error) {
  console.error(`오류: ${error.message}`);
  process.exit(1);
}
