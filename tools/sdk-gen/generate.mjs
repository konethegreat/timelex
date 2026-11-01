// Written by Kone Tshivhinda & Claude
// Generates packages/sdk/src/generated from the contracts. `--check` compares without writing.
// The generator lives alone (TypeScript 5) because openapi-typescript needs the classic compiler API,
// which TypeScript 7 no longer ships; the SDK itself builds with TypeScript 7.

import { mkdir, readFile, readdir, unlink, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { compileFromFile } from "json-schema-to-typescript";
import openapiTS, { astToString } from "openapi-typescript";

const repo = fileURLToPath(new URL("../../", import.meta.url));
const outDir = join(repo, "packages/sdk/src/generated");
const check = process.argv.includes("--check");

const OPENAPI = [
  { name: "gateway", source: "contracts/openapi/gateway.v1.yaml" },
  { name: "identity", source: "contracts/openapi/identity.v1.yaml" },
  { name: "vault", source: "contracts/openapi/vault.v1.yaml" },
];

const SCHEMAS = [
  { name: "activity-event", source: "contracts/events/activity-event.schema.json" },
];

const GENERATED_MARKER = "// Generated from";

function header(source) {
  return [
    "// Written by Kone Tshivhinda & Claude",
    `${GENERATED_MARKER} ${source} by tools/sdk-gen. Do not edit by hand.`,
    "// Run `npm run generate:sdk` after changing the contract; `npm run check:sdk` fails on drift.",
    "",
    "",
  ].join("\n");
}

async function renderOpenapi(target) {
  const ast = await openapiTS(pathToFileURL(join(repo, target.source)), {
    alphabetize: true,
    exportType: true,
  });
  return `${header(target.source)}${astToString(ast).trimEnd()}\n`;
}

async function renderSchema(target) {
  const body = await compileFromFile(join(repo, target.source), {
    bannerComment: "",
    style: { printWidth: 100, singleQuote: false },
  });
  return `${header(target.source)}${body.trimEnd()}\n`;
}

async function generatedFiles() {
  try {
    return (await readdir(outDir)).filter((file) => file.endsWith(".ts"));
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
}

async function isGenerated(file) {
  const text = await readFile(join(outDir, file), "utf8");
  return text.split("\n", 3)[1]?.includes(GENERATED_MARKER) === true;
}

async function main() {
  const outputs = new Map();
  for (const target of OPENAPI) {
    outputs.set(`${target.name}.ts`, await renderOpenapi(target));
  }
  for (const target of SCHEMAS) {
    outputs.set(`${target.name}.ts`, await renderSchema(target));
  }

  if (check) {
    const problems = [];
    for (const [file, expected] of outputs) {
      let actual;
      try {
        actual = await readFile(join(outDir, file), "utf8");
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
        problems.push(`${file}: missing`);
        continue;
      }
      if (actual.replaceAll("\r\n", "\n") !== expected) {
        problems.push(`${file}: differs from the contract`);
      }
    }
    for (const file of await generatedFiles()) {
      if (!outputs.has(file)) problems.push(`${file}: stale, no matching contract`);
    }
    if (problems.length > 0) {
      console.error("SDK types are out of date:");
      for (const problem of problems) console.error(`  ${problem}`);
      console.error("Run `npm run generate:sdk` and commit the result.");
      process.exitCode = 1;
      return;
    }
    console.log(`SDK types match ${outputs.size} contract files.`);
    return;
  }

  await mkdir(outDir, { recursive: true });
  for (const [file, content] of outputs) {
    await writeFile(join(outDir, file), content);
    console.log(`wrote ${relative(repo, join(outDir, file))}`);
  }
  for (const file of await generatedFiles()) {
    if (outputs.has(file) || !(await isGenerated(file))) continue;
    await unlink(join(outDir, file));
    console.log(`removed stale ${relative(repo, join(outDir, file))}`);
  }
}

await main();
