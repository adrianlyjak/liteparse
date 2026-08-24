import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

import { LiteParse } from "../dist/lib.js";

const fixture = fileURLToPath(
  new URL("../../../integration_tests_data/filled_acroform.pdf", import.meta.url),
);
const receipt = fileURLToPath(
  new URL("../../../integration_tests_data/receipt.png", import.meta.url),
);
const parser = new LiteParse({ ocrEnabled: false, quiet: true });
const oneShot = await parser.parsePages(fixture, [3, 1, 3]);
const document = await parser.openDocument(fixture);

try {
  assert.equal(document.pageCount, 3);
  const result = await document.parsePages([3, 1, 3]);
  assert.equal(result.totalPages, 3);
  assert.deepEqual(
    result.pages.map((page) => page.pageNum),
    [1, 3],
  );
  assert.equal(oneShot.totalPages, result.totalPages);
  assert.equal(oneShot.text, result.text);
  await document.reopen();
  assert.deepEqual(
    (await document.parsePages([3, 1, 3])).pages.map((page) => page.pageNum),
    [1, 3],
  );
  await assert.rejects(
    document.parsePages([1.5]),
    /page number must be a finite integer/,
  );
  await assert.rejects(
    document.parsePages([2 ** 32 + 1]),
    /page number must be a finite integer/,
  );
} finally {
  await document.close();
}

for (const [pages, message] of [
  [[], /page selection cannot be empty/],
  [[0], /page 0 out of range \(document has 3 pages\)/],
  [[1, 4], /page 4 out of range \(document has 3 pages\)/],
]) {
  await assert.rejects(parser.parsePages(fixture, pages), message);
}

// Arrays select the original synchronous projection overload. Buffer and
// Uint8Array inputs must dispatch to source parsing instead.
const bytes = await import("node:fs").then(({ readFile }) => readFile(fixture));
assert.deepEqual(
  (await parser.parsePages(bytes, [2])).pages.map((page) => page.pageNum),
  [2],
);
assert.deepEqual(
  (await parser.parsePages(new Uint8Array(bytes), [2])).pages.map(
    (page) => page.pageNum,
  ),
  [2],
);

await assert.rejects(document.parsePages([1]), /document is closed/);
await assert.rejects(document.reopen(), /document is closed/);

const converted = await parser.openDocument(receipt);
try {
  assert.equal(converted.pageCount, 1);
  assert.equal((await converted.parse()).totalPages, 1);
} finally {
  await converted.close();
}
