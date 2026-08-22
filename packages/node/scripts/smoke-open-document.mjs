import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";

import { LiteParse } from "../dist/lib.js";

const fixture = fileURLToPath(
  new URL("../../../integration_tests_data/filled_acroform.pdf", import.meta.url),
);
const receipt = fileURLToPath(
  new URL("../../../integration_tests_data/receipt.png", import.meta.url),
);
const parser = new LiteParse({
  ocrEnabled: false,
  quiet: true,
  dpi: 96,
  renderFormFields: true,
});
const oneShot = await parser.parsePages(fixture, [3, 1, 3]);
const oneShotScreenshots = await parser.screenshotPages(fixture, [3, 1, 3]);
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
  const screenshots = await document.screenshotPages([3, 1, 3]);
  assert.deepEqual(
    screenshots.map((page) => page.pageNum),
    [3, 1, 3],
  );
  assert.ok(screenshots.every((page) => page.imageBuffer.length > 8));
  assert.deepEqual(
    oneShotScreenshots.map((page) => page.pageNum),
    [3, 1, 3],
  );
  assert.deepEqual(
    oneShotScreenshots.map((page) => page.imageBuffer),
    screenshots.map((page) => page.imageBuffer),
  );
  await assert.rejects(
    document.screenshotPages([1.5]),
    /page number must be a finite integer/,
  );
  await assert.rejects(
    document.screenshotPages([2 ** 32 + 1]),
    /page number must be a finite integer/,
  );
  const raster = await document.rasterPage(1, {
    dpi: 36,
    pixelFormat: "rgb8",
  });
  assert.equal(raster.stride, raster.width * 3);
  assert.equal(raster.pixels.length, raster.stride * raster.height);
  const alignedRaster = await document.rasterPage(1, {
    dpi: 36,
    pixelFormat: "rgbx8",
  });
  assert.equal(alignedRaster.width, raster.width);
  assert.equal(alignedRaster.height, raster.height);
  assert.equal(alignedRaster.stride, alignedRaster.width * 4);
  assert.equal(
    alignedRaster.pixels.length,
    alignedRaster.stride * alignedRaster.height,
  );
  for (let pixel = 0; pixel < raster.width * raster.height; pixel += 1) {
    assert.deepEqual(
      alignedRaster.pixels.subarray(pixel * 4, pixel * 4 + 3),
      raster.pixels.subarray(pixel * 3, pixel * 3 + 3),
    );
    assert.equal(alignedRaster.pixels[pixel * 4 + 3], 255);
  }
  await assert.rejects(
    document.rasterPage(1.5),
    /page number must be a finite integer/,
  );
  await assert.rejects(
    document.rasterPage(2 ** 32 + 1),
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

for (const [pages, message] of [
  [[], /page selection cannot be empty/],
  [[0], /page 0 out of range \(document has 3 pages\)/],
  [[1, 4], /page 4 out of range \(document has 3 pages\)/],
]) {
  await assert.rejects(parser.screenshotPages(fixture, pages), message);

  const retained = await parser.openDocument(fixture);
  try {
    await assert.rejects(retained.screenshotPages(pages), message);
  } finally {
    await retained.close();
  }
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
await assert.rejects(document.screenshotPages([1]), /document is closed/);
await assert.rejects(document.rasterPage(1), /document is closed/);

const converted = await parser.openDocument(receipt);
try {
  assert.equal(converted.pageCount, 1);
  assert.equal((await converted.parse()).totalPages, 1);
} finally {
  await converted.close();
}
