import assert from "node:assert/strict";
import { join } from "node:path";
import test from "node:test";

import { loadFirstAvailable, nativeBindingCandidates } from "../dist/native-loader.js";

const searchDir = join("tmp", "liteparse");
const candidates = nativeBindingCandidates("linux", "x64", [searchDir]);
const localModule = join(searchDir, "liteparse.linux-x64-gnu.node");
const optionalPackage = "@llamaindex/liteparse-linux-x64-gnu";

test("a local native build wins over an installed optional package", () => {
  const attempts = [];
  const loaded = loadFirstAvailable(candidates, (candidate) => {
    attempts.push(candidate);
    if (candidate === localModule) return "local";
    if (candidate === optionalPackage) return "optional";
    throw new Error("not found");
  });

  assert.equal(loaded, "local");
  assert.deepEqual(attempts, [localModule]);
});

test("an installed optional package remains the fallback", () => {
  const loaded = loadFirstAvailable(candidates, (candidate) => {
    if (candidate === optionalPackage) return "optional";
    throw new Error("not found");
  });

  assert.equal(loaded, "optional");
});
