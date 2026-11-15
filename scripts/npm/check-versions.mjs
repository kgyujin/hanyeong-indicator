#!/usr/bin/env node
/*
 * 작성자: Git 이력 참조
 * 작성목적: 버전이 적힌 세 파일의 버전이 같은지 확인한다(CI에서 실행)
 * 작성일: 2026-11-15
 * 변경사항 내역:
 * - 2026-11-15 | npm 없이 npx | 버전 일치 검사 추가
 *
 * 왜 필요한가: npx 설치 명령은 자기 package.json 버전으로 GitHub 릴리스(v<버전>)의 실행 파일을 받는다.
 * Cargo.toml(릴리스 태그 기준)과 버전이 다르면 없는 릴리스를 찾거나 옛 버전을 설치한다.
 * 버전을 올릴 때는 Cargo.toml, package.json, npm/hanyeong-indicator/package.json을 함께 바꾼다.
 */
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const cargo = readFileSync(join(ROOT, "Cargo.toml"), "utf8").match(/^version = "(.+)"/m)?.[1];
const versions = {
  "Cargo.toml": cargo,
  "package.json": JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).version,
  "npm/hanyeong-indicator/package.json": JSON.parse(
    readFileSync(join(ROOT, "npm/hanyeong-indicator/package.json"), "utf8"),
  ).version,
};
const distinct = new Set(Object.values(versions));
if (distinct.size !== 1 || !cargo) {
  console.error("버전이 서로 다릅니다:");
  for (const [file, version] of Object.entries(versions)) console.error(`  ${file}: ${version}`);
  process.exit(1);
}
console.log(`버전 일치: ${cargo}`);
