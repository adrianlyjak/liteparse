import { join } from "node:path";

const platformPackages: Record<string, string> = {
  "darwin-x64": "@llamaindex/liteparse-darwin-x64",
  "darwin-arm64": "@llamaindex/liteparse-darwin-arm64",
  "linux-x64-gnu": "@llamaindex/liteparse-linux-x64-gnu",
  "linux-x64-musl": "@llamaindex/liteparse-linux-x64-musl",
  "linux-arm64-gnu": "@llamaindex/liteparse-linux-arm64-gnu",
  "linux-arm64-musl": "@llamaindex/liteparse-linux-arm64-musl",
  "win32-x64-msvc": "@llamaindex/liteparse-win32-x64-msvc",
  "win32-arm64-msvc": "@llamaindex/liteparse-win32-arm64-msvc",
};

export function nativeBindingCandidates(
  platform: NodeJS.Platform,
  arch: string,
  searchDirs: string[],
): string[] {
  const platformVariants =
    platform === "linux"
      ? [`${platform}-${arch}-gnu`, `${platform}-${arch}-musl`]
      : platform === "win32"
        ? [`${platform}-${arch}-msvc`]
        : [`${platform}-${arch}`];

  const fileNames = [
    ...platformVariants.map((variant) => `liteparse.${variant}.node`),
    `liteparse.${platform}-${arch}.node`,
    "liteparse.node",
  ];
  const localModules = searchDirs.flatMap((dir) =>
    fileNames.map((fileName) => join(dir, fileName)),
  );
  const optionalPackages = platformVariants.flatMap((variant) => {
    const packageName = platformPackages[variant];
    return packageName ? [packageName] : [];
  });

  return [...localModules, ...optionalPackages];
}

export function loadFirstAvailable<T>(
  candidates: string[],
  load: (candidate: string) => T,
): T | undefined {
  for (const candidate of candidates) {
    try {
      return load(candidate);
    } catch {
      // Try the next local build or installed platform package.
    }
  }

  return undefined;
}
