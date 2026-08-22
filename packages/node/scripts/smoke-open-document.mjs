import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

import { LiteParse } from "../dist/lib.js";

const fixture = fileURLToPath(
  new URL("../../../integration_tests_data/filled_acroform.pdf", import.meta.url),
);
const parser = new LiteParse({ ocrEnabled: false, quiet: true });
const document = await parser.openDocument(fixture);

try {
  assert.equal(document.pageCount, 3);
  const result = await document.parsePages([3, 1, 3]);
  assert.equal(result.totalPages, 3);
  assert.deepEqual(
    result.pages.map((page) => page.pageNum),
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

await assert.rejects(document.parsePages([1]), /document is closed/);
