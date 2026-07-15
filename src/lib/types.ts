export interface Rect {
  id: number;
  x: number;
  y: number;
  w: number;
  h: number;
  name: string;
  size: number;
  is_dir: boolean;
  ext: string;
  frac: number;
}

export interface BigFile {
  id: number;
  name: string;
  path: string;
  size: number;
}

export interface TypeSlice {
  ext: string;
  bytes: number;
  files: number;
  frac: number;
}

export interface Crumb {
  id: number;
  name: string;
}

export interface ScanSummary {
  root_path: string;
  files: number;
  bytes: number;
  skipped: number;
  nodes: number;
}

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${UNITS[unit]}`;
}

export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

// ---------------------------------------------------------------- palette
// warm mineral palette, keyed by extension family - deliberately no purple
const FAMILY_COLORS: Record<string, string> = {
  media: "#c76b4a", // copper   - video/audio
  image: "#cbb287", // sand     - images
  code: "#8aa37b",  // moss     - source code
  doc: "#6d93a8",   // slate    - documents
  archive: "#a8794e", // leather - archives/packages
  data: "#9aa38d",  // lichen   - data/db
  binary: "#7d8391", // gunmetal - executables/libs
  other: "#5d636e", // basalt
};

const EXT_FAMILY: Record<string, string> = {
  mp4: "media", mkv: "media", avi: "media", mov: "media", webm: "media",
  mp3: "media", wav: "media", flac: "media", ogg: "media", m4a: "media",
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image",
  svg: "image", ico: "image", bmp: "image", psd: "image",
  rs: "code", ts: "code", tsx: "code", js: "code", jsx: "code", py: "code",
  c: "code", h: "code", cpp: "code", java: "code", go: "code", css: "code",
  html: "code", json: "code", toml: "code", yaml: "code", yml: "code",
  pdf: "doc", doc: "doc", docx: "doc", txt: "doc", md: "doc", ppt: "doc",
  pptx: "doc", xls: "doc", xlsx: "doc", epub: "doc",
  zip: "archive", tar: "archive", gz: "archive", rar: "archive", "7z": "archive",
  deb: "archive", rpm: "archive", dmg: "archive", iso: "archive",
  db: "data", sqlite: "data", csv: "data", parquet: "data", npz: "data",
  pkl: "data", log: "data",
  exe: "binary", dll: "binary", so: "binary", dylib: "binary", bin: "binary",
  a: "binary", o: "binary", wasm: "binary",
};

export function colorFor(rect: Rect): string {
  if (rect.id === 4294967295) return "#3a3f47"; // the "small items" aggregate
  if (rect.is_dir) return "#22262c";            // dirs are recessed panels
  const family = EXT_FAMILY[rect.ext] ?? "other";
  return FAMILY_COLORS[family];
}

export function typeColor(ext: string): string {
  const family = EXT_FAMILY[ext] ?? "other";
  return FAMILY_COLORS[family] ?? FAMILY_COLORS.other;
}
