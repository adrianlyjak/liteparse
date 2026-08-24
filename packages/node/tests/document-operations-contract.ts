import type {
  DocumentOperations,
  LiteParse,
  LiteParseInput,
  OpenDocument,
  PageInput,
  ParseResult,
  ScreenshotResult,
} from "../src/lib.js";

declare const parser: LiteParse;
declare const document: OpenDocument;
declare const input: LiteParseInput;
declare const extractedPages: PageInput[];

const oneShotOperations: DocumentOperations<[input: LiteParseInput]> = parser;
const retainedOperations: DocumentOperations<[]> = document;

const oneShotParse: Promise<ParseResult> = oneShotOperations.parse(input);
const oneShotSelectedParse: Promise<ParseResult> = oneShotOperations.parsePages(
  input,
  [3, 1, 3],
);
const retainedParse: Promise<ParseResult> = retainedOperations.parse();
const retainedSelectedParse: Promise<ParseResult> =
  retainedOperations.parsePages([3, 1, 3]);
const oneShotScreenshots: Promise<ScreenshotResult[]> =
  oneShotOperations.screenshotPages(input, [3, 1, 3]);
const retainedScreenshots: Promise<ScreenshotResult[]> =
  retainedOperations.screenshotPages([3, 1, 3]);

// LiteParse keeps its existing synchronous projection overload and adds an
// async two-argument overload for selected source pages.
const projected: ParseResult = parser.parsePages(extractedPages);
const selected: Promise<ParseResult> = parser.parsePages(input, [3, 1, 3]);
const selectedScreenshots: Promise<ScreenshotResult[]> = parser.screenshotPages(
  input,
  [3, 1, 3],
);

void [
  oneShotParse,
  oneShotSelectedParse,
  retainedParse,
  retainedSelectedParse,
  oneShotScreenshots,
  retainedScreenshots,
  projected,
  selected,
  selectedScreenshots,
];
