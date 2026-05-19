import {
  openMcd,
  type Diagnostic,
  type DocumentBlock,
  type SourceSpan,
  type ValidationResult,
} from "@mcd/parser";
import DOMPurify from "dompurify";
import JSZip from "jszip";
import katex from "katex";
import { marked, type Tokens } from "marked";
import Papa from "papaparse";

import "katex/dist/katex.min.css";
import "./styles.css";

const MCD_MIMETYPE = "application/vnd.mcd+zip";
const UNSAVED_CHANGES_PROMPT = "Save changes?";
const DEFAULT_ENTRYPOINT = "content/main.md";
const HISTORY_LIMIT = 20;
const HISTORY_GROUP_IDLE_MS = 1200;
const EMPTY_FIRST_HEADING_ID = "mcd-empty-first-heading";
const RESERVED_ROW_HEADER_COLUMN = "row_header";
const textDecoder = new TextDecoder();
type ActiveTab = "text" | "tables" | "annotations";

marked.use({
  gfm: true,
  breaks: true,
  renderer: {
    code({ text, lang }: Tokens.Code): string | false {
      if (lang?.trim().toLowerCase() === "math") {
        return renderMath(text, true);
      }
      return false;
    },
  },
  extensions: [
    {
      name: "displayMath",
      level: "block",
      start(src: string): number | void {
        return src.match(/\$\$/)?.index;
      },
      tokenizer(src: string): Tokens.Generic | undefined {
        const match = /^\$\$[ \t]*\n?([\s\S]+?)\n?\$\$(?:\n+|$)/.exec(src);
        if (!match) {
          return undefined;
        }
        return {
          type: "displayMath",
          raw: match[0],
          text: match[1]?.trim() ?? "",
        };
      },
      renderer(token: Tokens.Generic): string {
        return renderMath(String(token.text ?? ""), true);
      },
    },
    {
      name: "inlineMath",
      level: "inline",
      start(src: string): number | void {
        const dollar = src.indexOf("$");
        const paren = src.indexOf("\\(");
        if (dollar === -1) {
          return paren === -1 ? undefined : paren;
        }
        if (paren === -1) {
          return dollar;
        }
        return Math.min(dollar, paren);
      },
      tokenizer(src: string): Tokens.Generic | undefined {
        const parenMatch = /^\\\(([\s\S]+?)\\\)/.exec(src);
        if (parenMatch) {
          return {
            type: "inlineMath",
            raw: parenMatch[0],
            text: parenMatch[1]?.trim() ?? "",
          };
        }

        const dollarMatch = /^\$(?!\s|\$)((?:\\.|[^\\$\n])+?)(?<!\s)\$(?!\$)/.exec(src);
        if (!dollarMatch) {
          return undefined;
        }
        return {
          type: "inlineMath",
          raw: dollarMatch[0],
          text: dollarMatch[1]?.trim() ?? "",
        };
      },
      renderer(token: Tokens.Generic): string {
        return renderMath(String(token.text ?? ""), false);
      },
    },
  ],
});

interface Manifest {
  format: "MCD";
  version: "0.1";
  profile: string;
  entrypoint: string;
  title?: string;
  tables?: TableManifestEntry[];
  images?: ImageManifestEntry[];
  annotations?: AnnotationManifestEntry[];
  assets?: AssetManifestEntry[];
  layout?: LayoutManifestEntry;
  [key: string]: unknown;
}

interface LayoutManifestEntry {
  styles?: string;
  pageMap?: string;
  [key: string]: unknown;
}

interface PageMap {
  pages: PageMapPage[];
}

interface PageMapPage {
  number: number;
  label?: string;
  sourceRefs?: string[];
  assets?: string[];
  rendered?: string;
}

interface TableManifestEntry {
  id: string;
  data: string;
  schema: string;
  views?: Record<string, string>;
}

interface ImageManifestEntry {
  id: string;
  metadata: string;
}

interface AssetManifestEntry {
  id?: string;
  path: string;
}

interface AnnotationManifestEntry {
  id: string;
  metadata: string;
}

interface TableColumn {
  name: string;
  type: string;
  label?: string;
  nullable?: boolean;
}

interface TableSchema {
  id: string;
  columns: TableColumn[];
}

interface TableViewColumn {
  name: string;
  label?: string;
  format?: string;
  currency?: string;
  unit?: string;
  percent?: boolean;
}

interface TableChartEncoding {
  column: string;
  label?: string;
  format?: string;
  currency?: string;
  unit?: string;
  percent?: boolean;
}

interface TableView {
  id: string;
  table: string;
  display?: "table" | "chart";
  columns?: TableViewColumn[];
  style?: TableViewStyle;
  chart?: {
    x?: TableChartEncoding;
    y?: TableChartEncoding;
    series?: TableChartEncoding;
    grouping?: TableChartEncoding;
    markLabels?: Partial<TableChartEncoding> & { show?: boolean };
  };
}

interface TableViewStyle {
  showColumnHeaders?: boolean;
  showRowHeaders?: boolean;
  [key: string]: unknown;
}

interface EditableTable {
  manifest: TableManifestEntry;
  schema: TableSchema;
  views: Record<string, TableView>;
  rows: Record<string, string>[];
}

interface TablePlacement {
  table: string;
  view?: string;
  display: "table" | "chart";
  source?: SourceSpan;
}

interface InsertLineTarget {
  body: HTMLDivElement;
  y: number;
}

type EditableTextBlock = Extract<
  DocumentBlock,
  { type: "heading" | "paragraph" | "list" | "quote" }
>;

interface InlineTableBinding {
  row: Record<string, string>;
  column: TableViewColumn & { label: string; schema: TableColumn };
}

interface InlineTextBinding {
  block?: EditableTextBlock;
  source?: SourceSpan;
  headingSplit?: InlineHeadingSplitBinding;
}

interface InlineHeadingSplitBinding {
  block: Extract<EditableTextBlock, { type: "heading" }>;
  source: SourceSpan;
  heading: HTMLElement;
  continuation: HTMLElement;
}

interface EditableAnnotation {
  id: string;
  metadata: string;
  targetText: string;
  page: string;
  line: string;
  kind: string;
  status: string;
  body: string;
  author: string;
  labels: string;
  created: string;
  originalMetadata?: string;
}

interface AnnotationPreviewItem {
  id: string;
  number: number;
  annotation: EditableAnnotation;
  line: number;
  hasInlineMarker: boolean;
}

interface PackageState {
  fileName: string;
  zip: JSZip;
  manifest: Manifest;
  markdown: string;
  tables: EditableTable[];
  annotations: EditableAnnotation[];
  pageMap?: PageMap;
  pageMapPath?: string;
  removedAnnotationPaths: Set<string>;
  dirty: boolean;
  plainMarkdownInput: boolean;
}

interface StateSnapshot {
  manifest: Manifest;
  markdown: string;
  tables: EditableTable[];
  annotations: EditableAnnotation[];
  pageMap?: PageMap;
  pageMapPath?: string;
  removedAnnotationPaths: string[];
}

interface SaveFilePickerWindow extends Window {
  showSaveFilePicker?: (options: {
    suggestedName?: string;
    types?: Array<{
      description: string;
      accept: Record<string, string[]>;
    }>;
  }) => Promise<{
    createWritable: () => Promise<{
      write: (data: Blob) => Promise<void>;
      close: () => Promise<void>;
    }>;
  }>;
}

let state: PackageState | undefined;
let activeTab: ActiveTab = "text";
let renderTimer: number | undefined;
let assetUrls: string[] = [];
let expandedAnnotationIds = new Set<string>();
let previewEditMode = false;
let sidebarExpanded = false;
let inlineTextBindings = new WeakMap<HTMLElement, InlineTextBinding>();
let inlineTableBindings = new WeakMap<HTMLElement, InlineTableBinding>();
let previewBlockSources = new WeakMap<HTMLElement, SourceSpan>();
let locallySavedAnnotationIds = new Set<string>();
let undoStack: StateSnapshot[] = [];
let redoStack: StateSnapshot[] = [];
let activeHistoryGroupKey: string | undefined;
let historyGroupTimer: number | undefined;
let savedContentKey = "";
let activeModal: HTMLElement | undefined;

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("Missing #app root.");
}

app.innerHTML = `
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <img class="brand-logo" src="/MCD_logo_tight.png" alt="MCD" />
        <div class="brand-title">Viewer</div>
      </div>
      <div class="file-name" id="fileName"></div>
      <div class="toolbar">
        <button id="openButton" type="button">Upload</button>
        <button id="createButton" type="button">Create</button>
        <button id="topEditModeButton" type="button" disabled aria-pressed="false">Edit</button>
        <button id="topSaveButton" class="primary" type="button" disabled>Save</button>
      </div>
    </header>
    <main class="workspace is-sidebar-folded" id="workspace">
      <section class="editor-pane" id="editorPane">
        <div class="sidebar-strip">
          <div class="sidebar-control-stack">
            <button id="sidebarToggle" class="sidebar-toggle" type="button" aria-expanded="false" aria-label="Unfold sidebar" title="Unfold sidebar">
              <span class="sidebar-toggle-icon" aria-hidden="true"></span>
            </button>
            <button id="undoButton" class="sidebar-action" type="button" aria-label="Undo" title="Undo" disabled>
              <span class="sidebar-history-icon is-undo" aria-hidden="true"></span>
            </button>
            <button id="redoButton" class="sidebar-action" type="button" aria-label="Redo" title="Redo" disabled>
              <span class="sidebar-history-icon is-redo" aria-hidden="true"></span>
            </button>
          </div>
        </div>
        <div class="editor-content">
          <input id="fileInput" class="hidden-input" type="file" accept=".mcd,application/zip,application/vnd.mcd+zip,text/markdown,text/plain" />
          <div class="status-panel">
            <div class="status-line" id="statusLine"></div>
            <div class="diagnostics" id="diagnostics"></div>
          </div>
          <nav class="tabs" aria-label="Editor sections">
            <button class="tab" id="tabText" type="button" aria-selected="true">Text</button>
            <button class="tab" id="tabTables" type="button" aria-selected="false">Tables</button>
            <button class="tab" id="tabAnnotations" type="button" aria-selected="false">Annotations</button>
          </nav>
          <section class="panel is-active" id="textPanel">
            <div class="field">
              <label for="markdownEditor">Markdown entrypoint</label>
              <textarea id="markdownEditor" spellcheck="false" disabled></textarea>
            </div>
          </section>
          <section class="panel" id="tablesPanel">
            <div class="list-stack" id="tablesEditor"></div>
          </section>
          <section class="panel" id="annotationsPanel">
            <div class="table-actions">
              <button id="addAnnotationButton" class="primary" type="button" disabled>Add annotation</button>
            </div>
            <div class="list-stack" id="annotationsEditor"></div>
          </section>
        </div>
      </section>
      <section class="preview-pane" id="previewPane">
        <article class="preview-document is-empty" id="preview">
          ${emptyDropZoneHtml()}
        </article>
      </section>
    </main>
    <div class="floating-actions" id="floatingActions" aria-label="Document actions" hidden>
      <button id="floatingEditModeButton" type="button" disabled aria-pressed="false">Edit</button>
      <button id="floatingSaveButton" class="primary" type="button" disabled>Save</button>
    </div>
  </div>
`;

const fileNameEl = byId<HTMLDivElement>("fileName");
const workspace = byId<HTMLElement>("workspace");
const fileInput = byId<HTMLInputElement>("fileInput");
const openButton = byId<HTMLButtonElement>("openButton");
const createButton = byId<HTMLButtonElement>("createButton");
const editModeButtons = [
  byId<HTMLButtonElement>("topEditModeButton"),
  byId<HTMLButtonElement>("floatingEditModeButton"),
];
const saveButtons = [
  byId<HTMLButtonElement>("topSaveButton"),
  byId<HTMLButtonElement>("floatingSaveButton"),
];
const sidebarToggle = byId<HTMLButtonElement>("sidebarToggle");
const foundSidebarStrip = sidebarToggle.closest<HTMLElement>(".sidebar-strip");
if (!foundSidebarStrip) {
  throw new Error("Missing sidebar strip.");
}
const sidebarStrip = foundSidebarStrip;
const undoButton = byId<HTMLButtonElement>("undoButton");
const redoButton = byId<HTMLButtonElement>("redoButton");
const statusLine = byId<HTMLDivElement>("statusLine");
const diagnosticsEl = byId<HTMLDivElement>("diagnostics");
const markdownEditor = byId<HTMLTextAreaElement>("markdownEditor");
const tablesEditor = byId<HTMLDivElement>("tablesEditor");
const annotationsEditor = byId<HTMLDivElement>("annotationsEditor");
const addAnnotationButton = byId<HTMLButtonElement>("addAnnotationButton");
const previewPane = byId<HTMLElement>("previewPane");
const preview = byId<HTMLElement>("preview");
const floatingActions = byId<HTMLDivElement>("floatingActions");

openButton.addEventListener("click", () => fileInput.click());
createButton.addEventListener("click", () => {
  void createDocument();
});
sidebarToggle.addEventListener("click", () => {
  setSidebarExpanded(!sidebarExpanded);
});
undoButton.addEventListener("click", () => {
  void undoOperation();
});
redoButton.addEventListener("click", () => {
  void redoOperation();
});

fileInput.addEventListener("change", () => {
  const file = fileInput.files?.[0];
  if (file) {
    void loadFile(file);
  }
  fileInput.value = "";
});

for (const button of editModeButtons) {
  button.addEventListener("click", () => {
    setPreviewEditMode(!previewEditMode);
  });
}

for (const button of saveButtons) {
  button.addEventListener("click", () => {
    void saveDocument();
  });
}

previewPane.addEventListener("scroll", syncFloatingActions);
window.addEventListener("scroll", syncFloatingActions);

window.addEventListener("beforeunload", (event) => {
  if (!state?.dirty) {
    return;
  }
  event.preventDefault();
  event.returnValue = UNSAVED_CHANGES_PROMPT;
});

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && activeModal) {
    closeActiveModal();
  }
});

preview.addEventListener("dragover", (event: DragEvent) => {
  if (state) return;
  const dropZone = preview.querySelector<HTMLDivElement>("#dropZone");
  if (!dropZone) return;
  event.preventDefault();
  dropZone.classList.add("is-active");
});

preview.addEventListener("dragleave", (event: DragEvent) => {
  if (state) return;
  const dropZone = preview.querySelector<HTMLDivElement>("#dropZone");
  if (!dropZone) return;
  if (!preview.contains(event.relatedTarget as Node)) {
    dropZone.classList.remove("is-active");
  }
});

preview.addEventListener("drop", (event: DragEvent) => {
  if (state) return;
  const dropZone = preview.querySelector<HTMLDivElement>("#dropZone");
  if (!dropZone) return;
  event.preventDefault();
  dropZone.classList.remove("is-active");
  const file = event.dataTransfer?.files[0];
  if (file) {
    void loadFile(file);
  }
});

preview.addEventListener("click", (event) => {
  const dropZone = (event.target as Element | null)?.closest<HTMLDivElement>("#dropZone");
  if (!state && dropZone && preview.contains(dropZone)) {
    fileInput.click();
    return;
  }

  const link = (event.target as Element | null)?.closest<HTMLAnchorElement>(
    'a[href^="#mcd-annotation"]',
  );
  if (!link) {
    return;
  }
  const targetId = link.getAttribute("href")?.slice(1);
  if (!targetId) {
    return;
  }
  const target = document.getElementById(targetId);
  if (!target || !preview.contains(target)) {
    return;
  }
  event.preventDefault();
  target.setAttribute("tabindex", "-1");
  target.scrollIntoView({ behavior: "smooth", block: "center" });
  target.focus({ preventScroll: true });
});

preview.addEventListener("keydown", (event: KeyboardEvent) => {
  if (state) return;
  const dropZone = (event.target as Element | null)?.closest<HTMLDivElement>("#dropZone");
  if (!dropZone || !preview.contains(dropZone)) {
    return;
  }
  if (event.key !== "Enter" && event.key !== " ") {
    return;
  }
  event.preventDefault();
  fileInput.click();
});

markdownEditor.addEventListener("input", () => {
  if (!state) {
    return;
  }
  if (state.markdown === markdownEditor.value) {
    return;
  }
  recordHistoryCheckpoint({ coalesceKey: "markdown-editor" });
  state.markdown = markdownEditor.value;
  markDirty();
});

for (const tab of ["Text", "Tables", "Annotations"] as const) {
  byId<HTMLButtonElement>(`tab${tab}`).addEventListener("click", () => {
    setActiveTab(tab.toLowerCase() as ActiveTab);
  });
}

addAnnotationButton.addEventListener("click", () => {
  if (!state) {
    return;
  }
  recordHistoryCheckpoint();
  const id = nextAnnotationId(state);
  state.annotations.push({
    id,
    metadata: `annotations/${id}.annotation.json`,
    targetText: JSON.stringify(sourceLineTarget(state.manifest.entrypoint, 1), null, 2),
    page: firstPageValue(state),
    line: "1",
    kind: "comment",
    status: "open",
    body: "New annotation",
    author: "",
    labels: "",
    created: new Date().toISOString(),
  });
  expandedAnnotationIds.add(id);
  renderAnnotationsEditor();
  markDirty();
});

function byId<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) {
    throw new Error(`Missing #${id}.`);
  }
  return element as T;
}

function captureStateSnapshot(): StateSnapshot | undefined {
  if (!state) {
    return undefined;
  }
  return {
    manifest: cloneJson(state.manifest),
    markdown: state.markdown,
    tables: cloneJson(state.tables),
    annotations: cloneJson(state.annotations),
    pageMap: state.pageMap ? cloneJson(state.pageMap) : undefined,
    pageMapPath: state.pageMapPath,
    removedAnnotationPaths: [...state.removedAnnotationPaths].sort(),
  };
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function snapshotKey(snapshot: StateSnapshot): string {
  return JSON.stringify(snapshot);
}

function contentKey(snapshot: StateSnapshot): string {
  return JSON.stringify({
    markdown: snapshot.markdown,
    tables: snapshot.tables,
    annotations: snapshot.annotations,
  });
}

function recordHistoryCheckpoint(options: { coalesceKey?: string } = {}): void {
  const snapshot = captureStateSnapshot();
  if (!snapshot) {
    return;
  }

  if (options.coalesceKey) {
    if (activeHistoryGroupKey === options.coalesceKey) {
      startHistoryGroup(options.coalesceKey);
      return;
    }
  } else {
    clearHistoryGroup();
  }

  pushHistorySnapshot(undoStack, snapshot);
  redoStack = [];
  if (options.coalesceKey) {
    startHistoryGroup(options.coalesceKey);
  }
  syncHistoryButtons();
}

function pushHistorySnapshot(stack: StateSnapshot[], snapshot: StateSnapshot): void {
  const previous = stack.at(-1);
  if (previous && snapshotKey(previous) === snapshotKey(snapshot)) {
    return;
  }
  stack.push(snapshot);
  if (stack.length > HISTORY_LIMIT) {
    stack.splice(0, stack.length - HISTORY_LIMIT);
  }
}

function startHistoryGroup(key: string): void {
  activeHistoryGroupKey = key;
  if (historyGroupTimer) {
    window.clearTimeout(historyGroupTimer);
  }
  historyGroupTimer = window.setTimeout(() => {
    if (activeHistoryGroupKey === key) {
      clearHistoryGroup();
    }
  }, HISTORY_GROUP_IDLE_MS);
}

function clearHistoryGroup(): void {
  activeHistoryGroupKey = undefined;
  if (historyGroupTimer) {
    window.clearTimeout(historyGroupTimer);
    historyGroupTimer = undefined;
  }
}

function resetHistory(): void {
  clearHistoryGroup();
  undoStack = [];
  redoStack = [];
  const snapshot = captureStateSnapshot();
  savedContentKey = snapshot ? contentKey(snapshot) : "";
  syncHistoryButtons();
}

async function undoOperation(): Promise<void> {
  if (!state || undoStack.length === 0) {
    return;
  }
  const current = captureStateSnapshot();
  const previous = undoStack.pop();
  if (!current || !previous) {
    return;
  }
  pushHistorySnapshot(redoStack, current);
  await restoreStateSnapshot(previous);
}

async function redoOperation(): Promise<void> {
  if (!state || redoStack.length === 0) {
    return;
  }
  const current = captureStateSnapshot();
  const next = redoStack.pop();
  if (!current || !next) {
    return;
  }
  pushHistorySnapshot(undoStack, current);
  await restoreStateSnapshot(next);
}

async function restoreStateSnapshot(snapshot: StateSnapshot): Promise<void> {
  if (!state) {
    return;
  }
  clearHistoryGroup();
  state.manifest = cloneJson(snapshot.manifest);
  state.markdown = snapshot.markdown;
  state.tables = cloneJson(snapshot.tables);
  state.annotations = cloneJson(snapshot.annotations);
  state.pageMap = snapshot.pageMap ? cloneJson(snapshot.pageMap) : undefined;
  state.pageMapPath = snapshot.pageMapPath;
  state.removedAnnotationPaths = new Set(snapshot.removedAnnotationPaths);

  const annotationIds = new Set(state.annotations.map((annotation) => annotation.id));
  expandedAnnotationIds = new Set([...expandedAnnotationIds].filter((id) => annotationIds.has(id)));
  locallySavedAnnotationIds = new Set(
    [...locallySavedAnnotationIds].filter((id) => annotationIds.has(id)),
  );

  const restored = captureStateSnapshot();
  state.dirty = restored ? contentKey(restored) !== savedContentKey : true;
  hydrateUiFromState();
  await renderAndValidate();
  syncHistoryButtons();
}

function syncHistoryButtons(): void {
  const hasState = Boolean(state);
  undoButton.disabled = !hasState || undoStack.length === 0;
  redoButton.disabled = !hasState || redoStack.length === 0;
}

async function loadFile(file: File): Promise<void> {
  setStatus(`Opening ${file.name}...`);
  clearDiagnostics();
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    state = await loadPackage(file.name, bytes);
    expandedAnnotationIds = new Set();
    locallySavedAnnotationIds = new Set();
    resetHistory();
    previewPane.scrollTop = 0;
    window.scrollTo({ top: 0, left: 0 });
    hydrateUiFromState();
    await renderAndValidate();
  } catch (error) {
    state = undefined;
    expandedAnnotationIds = new Set();
    locallySavedAnnotationIds = new Set();
    resetHistory();
    hydrateUiFromState();
    showError(error);
  }
}

async function createDocument(): Promise<void> {
  setStatus("Creating empty MCD document...");
  clearDiagnostics();
  state = createDefaultPackageState();
  expandedAnnotationIds = new Set();
  locallySavedAnnotationIds = new Set();
  previewEditMode = false;
  resetHistory();
  previewPane.scrollTop = 0;
  window.scrollTo({ top: 0, left: 0 });
  setActiveTab("text");
  setSidebarExpanded(true);
  hydrateUiFromState();
  await renderAndValidate();
  markdownEditor.focus();
  setStatus("Created empty MCD document.");
}

function createDefaultPackageState(): PackageState {
  const zip = new JSZip();
  const manifest: Manifest = {
    format: "MCD",
    version: "0.1",
    profile: "MCD-Core",
    entrypoint: DEFAULT_ENTRYPOINT,
    tables: [],
    images: [],
    annotations: [],
    assets: [],
  };

  zip.file("mimetype", `${MCD_MIMETYPE}\n`, { compression: "STORE" });
  zip.file("manifest.json", `${JSON.stringify(manifest, null, 2)}\n`);
  zip.file(DEFAULT_ENTRYPOINT, "");
  zip.folder("tables");
  zip.folder("images");
  zip.folder("assets");

  return {
    fileName: "untitled.mcd",
    zip,
    manifest,
    markdown: "",
    tables: [],
    annotations: [],
    removedAnnotationPaths: new Set(),
    dirty: false,
    plainMarkdownInput: false,
  };
}

async function loadPackage(fileName: string, bytes: Uint8Array): Promise<PackageState> {
  let zip: JSZip;
  let plainMarkdownInput = false;

  try {
    zip = await JSZip.loadAsync(bytes);
  } catch {
    plainMarkdownInput = true;
    zip = new JSZip();
    zip.file("mimetype", `${MCD_MIMETYPE}\n`, { compression: "STORE" });
    zip.file(
      "manifest.json",
      JSON.stringify(
        {
          format: "MCD",
          version: "0.1",
          profile: "MCD-Core",
          entrypoint: DEFAULT_ENTRYPOINT,
        },
        null,
        2,
      ),
    );
    zip.file(DEFAULT_ENTRYPOINT, textDecoder.decode(bytes));
  }

  const manifest = await readManifest(zip);
  const markdown = await readText(zip, manifest.entrypoint);
  const tables = await readTables(zip, manifest.tables ?? []);
  const { pageMap, pageMapPath } = await readPageMap(zip, manifest);
  const annotations = await readAnnotations(
    zip,
    manifest.annotations ?? [],
    manifest.entrypoint,
    markdown,
    pageMap,
  );

  return {
    fileName,
    zip,
    manifest,
    markdown,
    tables,
    annotations,
    pageMap,
    pageMapPath,
    removedAnnotationPaths: new Set(),
    dirty: false,
    plainMarkdownInput,
  };
}

async function readManifest(zip: JSZip): Promise<Manifest> {
  const manifestText = await readText(zip, "manifest.json");
  return JSON.parse(manifestText) as Manifest;
}

async function readTables(
  zip: JSZip,
  entries: TableManifestEntry[],
): Promise<EditableTable[]> {
  const tables: EditableTable[] = [];
  for (const entry of entries) {
    const schema = JSON.parse(await readText(zip, entry.schema)) as TableSchema;
    const csv = await readText(zip, entry.data);
    tables.push({
      manifest: entry,
      schema,
      views: await readTableViews(zip, entry),
      rows: parseCsvRows(csv, schema.columns),
    });
  }
  return tables;
}

async function readTableViews(
  zip: JSZip,
  entry: TableManifestEntry,
): Promise<Record<string, TableView>> {
  const views: Record<string, TableView> = {};
  for (const [id, path] of Object.entries(entry.views ?? {})) {
    const file = zip.file(path);
    if (!file) {
      continue;
    }
    const view = JSON.parse(await file.async("string")) as TableView;
    views[id] = view;
  }
  return views;
}

async function readAnnotations(
  zip: JSZip,
  entries: AnnotationManifestEntry[],
  entrypoint: string,
  markdown: string,
  pageMap?: PageMap,
): Promise<EditableAnnotation[]> {
  const annotations: EditableAnnotation[] = [];
  for (const entry of entries) {
    const raw = JSON.parse(await readText(zip, entry.metadata)) as Record<string, unknown>;
    const target = targetRecord(raw.target);
    const line = targetSourceLine(target, entrypoint) ?? annotationMarkerLine(markdown, entry.id);
    const targetText =
      line && target?.type === "path" && target.path === entrypoint
        ? JSON.stringify(sourceLineTarget(entrypoint, line), null, 2)
        : JSON.stringify(target ?? { type: "document" }, null, 2);
    annotations.push({
      id: String(raw.id ?? entry.id),
      metadata: entry.metadata,
      targetText,
      page: line ? inferPageForLine(markdown, line, pageMap) : "",
      line: line?.toString() ?? "",
      kind: String(raw.kind ?? "comment"),
      status: String(raw.status ?? "open"),
      body: String(raw.body ?? ""),
      author: String(raw.author ?? ""),
      labels: Array.isArray(raw.labels) ? raw.labels.join(", ") : "",
      created: String(raw.created ?? ""),
      originalMetadata: entry.metadata,
    });
  }
  return annotations;
}

function annotationMarkerLine(markdown: string, id: string): number | undefined {
  const marker = `[[annotation:${id}]]`;
  const lines = markdown.split(/\r\n|\r|\n/);
  const index = lines.findIndex((line) => line.includes(marker));
  return index >= 0 ? index + 1 : undefined;
}

function targetRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function targetSourceLine(target: Record<string, unknown> | undefined, entrypoint: string): number | undefined {
  if (!target || target.type !== "path" || target.path !== entrypoint) {
    return undefined;
  }
  const source = targetRecord(target.source);
  const line = Number(source?.startLine);
  return Number.isInteger(line) && line > 0 ? line : undefined;
}

function sourceLineTarget(path: string, line: number): Record<string, unknown> {
  return {
    type: "path",
    path,
    source: {
      startLine: line,
      startColumn: 1,
      endLine: line,
      endColumn: 1,
    },
  };
}

function markdownLineCount(markdown: string): number {
  return Math.max(1, markdown.split(/\r\n|\r|\n/).length);
}

function firstPageValue(packageState: PackageState): string {
  return String(pageChoices(packageState)[0]?.number ?? 1);
}

function pageChoices(packageState: PackageState): Array<{ number: number; label: string }> {
  const pages = (packageState.pageMap?.pages ?? []).filter(
    (page) => page.label?.toLowerCase() !== "annotations",
  );
  if (pages.length > 0) {
    return pages.map((page) => ({
      number: page.number,
      label: page.label ?? `Page ${page.number}`,
    }));
  }
  return [{ number: 1, label: "Page 1" }];
}

function annotationPageOptions(selected: string): string {
  if (!state) {
    return options(["1"], selected || "1");
  }
  return pageChoices(state)
    .map((page) => {
      const value = String(page.number);
      const selectedAttr = value === selected ? " selected" : "";
      return `<option value="${escapeAttr(value)}"${selectedAttr}>${escapeHtml(page.label)}</option>`;
    })
    .join("");
}

function normalizeLineInput(value: string, markdown: string): string {
  const line = Number(value);
  if (!Number.isInteger(line) || line < 1) {
    return "";
  }
  return String(Math.min(line, markdownLineCount(markdown)));
}

function inferPageForLine(markdown: string, line: number, pageMap?: PageMap): string {
  const starts = pageStartLines(markdown, pageMap);
  const match = starts
    .filter((start) => start.line <= line)
    .sort((left, right) => right.line - left.line)[0];
  if (match) {
    return String(match.page);
  }

  const pageCount = Math.max(1, pageMap?.pages.length ?? 1);
  const approximate = Math.ceil((line / markdownLineCount(markdown)) * pageCount);
  return String(Math.min(Math.max(1, approximate), pageCount));
}

function firstLineForPage(markdown: string, page: number, pageMap?: PageMap): number {
  const start = pageStartLines(markdown, pageMap).find((entry) => entry.page === page);
  if (start) {
    return start.line;
  }

  const pageCount = Math.max(1, pageMap?.pages.length ?? 1);
  const lineCount = markdownLineCount(markdown);
  return Math.max(1, Math.floor(((page - 1) / pageCount) * lineCount) + 1);
}

function pageStartLines(markdown: string, pageMap?: PageMap): Array<{ page: number; line: number }> {
  const lines = markdown.split(/\r\n|\r|\n/);
  const pageNumbers = (pageMap?.pages ?? [{ number: 1 }]).map((page) => page.number);
  const starts: Array<{ page: number; line: number }> = [];

  for (const page of pageNumbers) {
    const headingPattern = new RegExp(`^#{1,6}\\s+Page\\s+0?${page}(?:\\b|:)`, "i");
    const headingIndex = lines.findIndex((line) => headingPattern.test(line.trim()));
    if (headingIndex >= 0) {
      starts.push({ page, line: headingIndex + 1 });
    }
  }

  if (starts.length > 0) {
    return starts.sort((left, right) => left.line - right.line);
  }

  return [{ page: pageNumbers[0] ?? 1, line: 1 }];
}

async function readPageMap(
  zip: JSZip,
  manifest: Manifest,
): Promise<{ pageMap?: PageMap; pageMapPath?: string }> {
  const pageMapPath = manifest.layout?.pageMap;
  if (!pageMapPath) {
    return {};
  }
  const file = zip.file(pageMapPath);
  if (!file) {
    return { pageMapPath };
  }
  const pageMap = JSON.parse(await file.async("string")) as PageMap;
  return { pageMap, pageMapPath };
}

async function readText(zip: JSZip, path: string): Promise<string> {
  const file = zip.file(path);
  if (!file) {
    throw new Error(`Package entry '${path}' is missing.`);
  }
  return file.async("string");
}

function parseCsvRows(csv: string, columns: TableColumn[]): Record<string, string>[] {
  const parsed = Papa.parse<string[]>(csv, {
    skipEmptyLines: "greedy",
  });
  const records = parsed.data;
  if (records.length === 0) {
    return [];
  }
  const headers = records[0] ?? [];
  return records.slice(1).map((record) => {
    const row: Record<string, string> = {};
    for (const column of columns) {
      const headerIndex = headers.indexOf(column.name);
      row[column.name] = headerIndex >= 0 ? (record[headerIndex] ?? "") : "";
    }
    return row;
  });
}

function hydrateUiFromState(): void {
  const hasState = Boolean(state);
  fileNameEl.textContent = state
    ? `${state.fileName}${state.dirty ? " (edited)" : ""}`
    : "";
  markdownEditor.disabled = !hasState;
  for (const button of editModeButtons) {
    button.disabled = !hasState;
  }
  for (const button of saveButtons) {
    button.disabled = !hasState;
  }
  addAnnotationButton.disabled = !hasState;
  if (!hasState) {
    previewEditMode = false;
  }
  syncEditModeButton();
  markdownEditor.value = state?.markdown ?? "";
  renderTablesEditor();
  renderAnnotationsEditor();
  preview.classList.toggle("is-empty", !hasState);
  syncFloatingActions();
  syncHistoryButtons();
  if (!state) {
    setStatus("");
    preview.innerHTML = emptyDropZoneHtml();
  }
}

function emptyDropZoneHtml(): string {
  return `<div id="dropZone" class="drop-zone" role="button" tabindex="0" aria-label="Upload or drop an MCD file">
    <div class="drop-title">Click to upload or drop a .mcd file here</div>
    <div class="drop-copy">The file is parsed locally in this browser session.</div>
  </div>`;
}

function setActiveTab(tab: ActiveTab): void {
  activeTab = tab;
  for (const name of ["text", "tables", "annotations"] as const) {
    const selected = name === tab;
    byId<HTMLButtonElement>(`tab${capitalize(name)}`).setAttribute(
      "aria-selected",
      selected ? "true" : "false",
    );
    byId<HTMLElement>(`${name}Panel`).classList.toggle("is-active", selected);
  }
}

function setSidebarExpanded(expanded: boolean): void {
  sidebarExpanded = expanded;
  workspace.classList.toggle("is-sidebar-folded", !expanded);
  workspace.classList.toggle("is-sidebar-expanded", expanded);
  sidebarToggle.setAttribute("aria-expanded", expanded ? "true" : "false");
  sidebarToggle.setAttribute("aria-label", expanded ? "Fold sidebar" : "Unfold sidebar");
  sidebarToggle.title = expanded ? "Fold sidebar" : "Unfold sidebar";
}

function syncFloatingActions(): void {
  const hasScrolledDocument = previewPane.scrollTop > 80 || window.scrollY > 80;
  const isVisible = Boolean(state) && hasScrolledDocument;
  floatingActions.hidden = !isVisible;
  floatingActions.classList.toggle("is-visible", isVisible);
  sidebarStrip.classList.toggle("is-pinned", hasScrolledDocument);
}

function renderTablesEditor(): void {
  tablesEditor.innerHTML = "";
  if (!state) {
    tablesEditor.innerHTML = `<div class="empty-state">No document loaded.</div>`;
    return;
  }
  if (state.tables.length === 0) {
    tablesEditor.innerHTML = `<div class="empty-state">This document does not declare CSV-backed tables.</div>`;
    return;
  }

  for (const table of state.tables) {
    const card = document.createElement("section");
    card.className = "item-card";
    const title = table.schema.id || table.manifest.id;
    card.innerHTML = `
      <div class="item-header">
        <div class="item-title">${escapeHtml(title)}</div>
        <span class="file-name">${escapeHtml(table.manifest.data)}</span>
      </div>
      <div class="table-wrap"></div>
      <div class="table-actions">
        <button type="button" data-action="add-row">Add row</button>
      </div>
    `;
    const tableWrap = card.querySelector<HTMLDivElement>(".table-wrap");
    if (!tableWrap) {
      throw new Error("Missing table wrapper.");
    }
    tableWrap.appendChild(renderTableGrid(table));
    card
      .querySelector<HTMLButtonElement>('[data-action="add-row"]')
      ?.addEventListener("click", () => {
        recordHistoryCheckpoint();
        const row = Object.fromEntries(
          table.schema.columns.map((column) => [
            column.name,
            column.name === RESERVED_ROW_HEADER_COLUMN ? String(table.rows.length + 1) : "",
          ]),
        );
        table.rows.push(row);
        renderTablesEditor();
        markDirty();
      });
    tablesEditor.appendChild(card);
  }
}

function renderTableGrid(table: EditableTable): HTMLTableElement {
  const grid = document.createElement("table");
  grid.className = "data-table";
  const thead = document.createElement("thead");
  const header = document.createElement("tr");
  for (const column of table.schema.columns) {
    const th = document.createElement("th");
    th.textContent = column.label ? `${column.label} (${column.name})` : column.name;
    header.appendChild(th);
  }
  const actionTh = document.createElement("th");
  actionTh.textContent = "";
  header.appendChild(actionTh);
  thead.appendChild(header);
  grid.appendChild(thead);

  const tbody = document.createElement("tbody");
  table.rows.forEach((row, rowIndex) => {
    const tr = document.createElement("tr");
    for (const column of table.schema.columns) {
      const td = document.createElement("td");
      const input = document.createElement("input");
      input.value = row[column.name] ?? "";
      input.setAttribute("aria-label", `${table.manifest.id} ${column.name} row ${rowIndex + 1}`);
      input.addEventListener("input", () => {
        if ((row[column.name] ?? "") === input.value) {
          return;
        }
        recordHistoryCheckpoint({
          coalesceKey: `table:${table.manifest.id}:${rowIndex}:${column.name}`,
        });
        row[column.name] = input.value;
        markDirty();
      });
      td.appendChild(input);
      tr.appendChild(td);
    }
    const actionTd = document.createElement("td");
    const remove = document.createElement("button");
    remove.className = "danger";
    remove.type = "button";
    remove.textContent = "Remove";
    remove.addEventListener("click", () => {
      recordHistoryCheckpoint();
      table.rows.splice(rowIndex, 1);
      renderTablesEditor();
      markDirty();
    });
    actionTd.appendChild(remove);
    tr.appendChild(actionTd);
    tbody.appendChild(tr);
  });
  grid.appendChild(tbody);
  return grid;
}

function renderAnnotationsEditor(): void {
  annotationsEditor.innerHTML = "";
  if (!state) {
    annotationsEditor.innerHTML = `<div class="empty-state">No document loaded.</div>`;
    return;
  }
  if (state.annotations.length === 0) {
    annotationsEditor.innerHTML = `<div class="empty-state">This document has no manifest-declared annotations.</div>`;
    return;
  }

  const packageState = state;
  state.annotations.forEach((annotation, index) => {
    const expanded = expandedAnnotationIds.has(annotation.id);
    const panelId = `annotation-panel-${index}-${sanitizeId(annotation.id)}`;
    const annotationHistoryKey = annotation.originalMetadata ?? annotation.metadata ?? annotation.id;
    const card = document.createElement("section");
    card.className = `item-card annotation-card${expanded ? " is-expanded" : ""}`;
    card.innerHTML = `
      <div class="item-header">
        <button class="annotation-summary" type="button" data-field="toggle" aria-expanded="${expanded}" aria-controls="${escapeAttr(
          panelId,
        )}">
          <span class="item-title">${escapeHtml(annotation.id)}</span>
        </button>
        <div class="item-actions">
          <button class="disclosure-button" type="button" data-field="toggle" aria-label="${
            expanded ? "Collapse annotation" : "Expand annotation"
          }" aria-expanded="${expanded}" aria-controls="${escapeAttr(panelId)}">
            <span class="disclosure-icon" aria-hidden="true"></span>
          </button>
        </div>
      </div>
      ${expanded ? annotationDetailsHtml(annotation, packageState, panelId) : ""}
    `;

    bindAnnotationInput(card, annotation, "id", annotationHistoryKey, (value) => {
      const previousId = annotation.id;
      const previous = annotation.metadata;
      annotation.id = sanitizeId(value);
      annotation.metadata = `annotations/${annotation.id}.annotation.json`;
      if (previous !== annotation.metadata) {
        state?.removedAnnotationPaths.add(previous);
      }
      if (expandedAnnotationIds.delete(previousId)) {
        expandedAnnotationIds.add(annotation.id);
      }
      if (locallySavedAnnotationIds.delete(previousId)) {
        locallySavedAnnotationIds.add(annotation.id);
      }
    });
    bindAnnotationInput(card, annotation, "kind", annotationHistoryKey, (value) => {
      annotation.kind = value;
    });
    bindAnnotationInput(card, annotation, "status", annotationHistoryKey, (value) => {
      annotation.status = value;
    });
    bindAnnotationInput(card, annotation, "author", annotationHistoryKey, (value) => {
      annotation.author = value;
    });
    bindAnnotationInput(card, annotation, "body", annotationHistoryKey, (value) => {
      annotation.body = value;
    });
    bindAnnotationInput(card, annotation, "page", annotationHistoryKey, (value) => {
      annotation.page = value;
      if (state && value) {
        annotation.line = firstLineForPage(state.markdown, Number(value), state.pageMap).toString();
        const lineInput = card.querySelector<HTMLInputElement>('[data-field="line"]');
        if (lineInput) {
          lineInput.value = annotation.line;
        }
      }
      updateAnnotationTargetFromLocation(annotation);
      updateAnnotationTargetTextarea(card, annotation);
    });
    bindAnnotationInput(card, annotation, "line", annotationHistoryKey, (value) => {
      annotation.line = normalizeLineInput(value, state?.markdown ?? "");
      if (state && annotation.line) {
        annotation.page = inferPageForLine(state.markdown, Number(annotation.line), state.pageMap);
        const pageInput = card.querySelector<HTMLSelectElement>('[data-field="page"]');
        if (pageInput) {
          pageInput.value = annotation.page;
        }
      }
      updateAnnotationTargetFromLocation(annotation);
      updateAnnotationTargetTextarea(card, annotation);
    });
    bindAnnotationInput(card, annotation, "targetText", annotationHistoryKey, (value) => {
      annotation.targetText = value;
      syncAnnotationLocationFromTarget(annotation);
      updateAnnotationLocationInputs(card, annotation);
    });
    bindAnnotationInput(card, annotation, "labels", annotationHistoryKey, (value) => {
      annotation.labels = value;
    });
    bindAnnotationInput(card, annotation, "created", annotationHistoryKey, (value) => {
      annotation.created = value;
    });
    card
      .querySelector<HTMLButtonElement>('[data-field="save"]')
      ?.addEventListener("click", () => {
        void saveAnnotationLocally(annotation);
      });
    for (const toggle of Array.from(
      card.querySelectorAll<HTMLButtonElement>('[data-field="toggle"]'),
    )) {
      toggle.addEventListener("click", () => {
        if (expandedAnnotationIds.has(annotation.id)) {
          expandedAnnotationIds.delete(annotation.id);
          locallySavedAnnotationIds.delete(annotation.id);
        } else {
          expandedAnnotationIds.add(annotation.id);
        }
        renderAnnotationsEditor();
      });
    }
    card
      .querySelector<HTMLButtonElement>('[data-field="remove"]')
      ?.addEventListener("click", () => {
        recordHistoryCheckpoint();
        state?.removedAnnotationPaths.add(annotation.metadata);
        if (annotation.originalMetadata) {
          state?.removedAnnotationPaths.add(annotation.originalMetadata);
        }
        expandedAnnotationIds.delete(annotation.id);
        locallySavedAnnotationIds.delete(annotation.id);
        state?.annotations.splice(index, 1);
        renderAnnotationsEditor();
        markDirty();
      });
    annotationsEditor.appendChild(card);
  });
}

function annotationDetailsHtml(
  annotation: EditableAnnotation,
  packageState: PackageState,
  panelId: string,
): string {
  return `
    <div class="annotation-details" id="${escapeAttr(panelId)}">
      <div class="annotation-detail-actions">
        ${annotationSaveButtonHtml(annotation)}
        <button class="danger" type="button" data-field="remove">Delete</button>
      </div>
      <div class="compact-row">
        <div class="field">
          <label>ID</label>
          <input data-field="id" value="${escapeAttr(annotation.id)}" />
        </div>
        <div class="field">
          <label>Kind</label>
          <select data-field="kind">
            ${options(["comment", "flag", "proposed_change", "question", "todo"], annotation.kind)}
          </select>
        </div>
      </div>
      <div class="compact-row">
        <div class="field">
          <label>Status</label>
          <select data-field="status">
            ${options(["open", "accepted", "rejected", "resolved"], annotation.status)}
          </select>
        </div>
        <div class="field">
          <label>Author</label>
          <input data-field="author" value="${escapeAttr(annotation.author)}" />
        </div>
      </div>
      <div class="field">
        <label>Body</label>
        <textarea data-field="body">${escapeHtml(annotation.body)}</textarea>
      </div>
      <div class="compact-row">
        <div class="field">
          <label>Page</label>
          <select data-field="page">
            ${annotationPageOptions(annotation.page)}
          </select>
        </div>
        <div class="field">
          <label>Line</label>
          <input data-field="line" type="number" min="1" max="${markdownLineCount(
            packageState.markdown,
          )}" value="${escapeAttr(annotation.line)}" />
        </div>
      </div>
      <div class="field">
        <label>Target JSON</label>
        <textarea data-field="targetText">${escapeHtml(annotation.targetText)}</textarea>
      </div>
      <div class="compact-row">
        <div class="field">
          <label>Labels</label>
          <input data-field="labels" value="${escapeAttr(annotation.labels)}" />
        </div>
        <div class="field">
          <label>Created</label>
          <input data-field="created" value="${escapeAttr(annotation.created)}" />
        </div>
      </div>
    </div>
  `;
}

function annotationSaveButtonHtml(annotation: EditableAnnotation): string {
  if (locallySavedAnnotationIds.has(annotation.id)) {
    return `<button class="primary annotation-save-button is-saved" type="button" data-field="save">
      <span aria-hidden="true">✓</span>
      <span>Saved</span>
    </button>`;
  }
  return `<button class="primary annotation-save-button" type="button" data-field="save">Save</button>`;
}

function bindAnnotationInput(
  root: HTMLElement,
  annotation: EditableAnnotation,
  field: keyof EditableAnnotation,
  historyKey: string,
  update: (value: string) => void,
): void {
  const input = root.querySelector<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
    `[data-field="${field}"]`,
  );
  if (!input) {
    return;
  }
  const handleChange = () => {
    const before = captureStateSnapshot();
    const undoLengthBefore = undoStack.length;
    recordHistoryCheckpoint({
      coalesceKey: `annotation:${historyKey}:${String(field)}`,
    });
    update(input.value);
    const after = captureStateSnapshot();
    if (before && after && snapshotKey(before) === snapshotKey(after)) {
      if (undoStack.length > undoLengthBefore) {
        undoStack.pop();
      }
      syncHistoryButtons();
      return;
    }
    if (field === "id") {
      const title = root.querySelector<HTMLDivElement>(".item-title");
      if (title) {
        title.textContent = annotation.id;
      }
    }
    locallySavedAnnotationIds.delete(annotation.id);
    resetAnnotationSaveButton(root);
    markDirty();
  };
  if (input instanceof HTMLSelectElement) {
    input.addEventListener("change", handleChange);
  } else {
    input.addEventListener("input", handleChange);
  }
}

function resetAnnotationSaveButton(root: HTMLElement): void {
  const button = root.querySelector<HTMLButtonElement>('[data-field="save"]');
  if (!button?.classList.contains("is-saved")) {
    return;
  }
  button.classList.remove("is-saved");
  button.textContent = "Save";
}

function updateAnnotationTargetFromLocation(annotation: EditableAnnotation): void {
  if (!state || !annotation.line) {
    return;
  }
  const line = Number(annotation.line);
  if (!Number.isInteger(line) || line < 1) {
    return;
  }
  annotation.targetText = JSON.stringify(sourceLineTarget(state.manifest.entrypoint, line), null, 2);
}

function syncAnnotationLocationFromTarget(annotation: EditableAnnotation): void {
  if (!state) {
    return;
  }
  try {
    const target = targetRecord(JSON.parse(annotation.targetText));
    const line = targetSourceLine(target, state.manifest.entrypoint);
    annotation.line = line?.toString() ?? "";
    annotation.page = line ? inferPageForLine(state.markdown, line, state.pageMap) : "";
  } catch {
    // Keep the user's in-progress JSON edit intact until it parses.
  }
}

function updateAnnotationTargetTextarea(root: HTMLElement, annotation: EditableAnnotation): void {
  const input = root.querySelector<HTMLTextAreaElement>('[data-field="targetText"]');
  if (input) {
    input.value = annotation.targetText;
  }
}

function updateAnnotationLocationInputs(root: HTMLElement, annotation: EditableAnnotation): void {
  const pageInput = root.querySelector<HTMLSelectElement>('[data-field="page"]');
  const lineInput = root.querySelector<HTMLInputElement>('[data-field="line"]');
  if (pageInput) {
    pageInput.value = annotation.page;
  }
  if (lineInput) {
    lineInput.value = annotation.line;
  }
}

function markDirty(options: { render?: boolean } = {}): void {
  if (!state) {
    return;
  }
  state.dirty = true;
  fileNameEl.textContent = `${state.fileName} (edited)`;
  syncHistoryButtons();
  if (options.render !== false) {
    queueRender();
  }
}

function setPreviewEditMode(enabled: boolean): void {
  if (!state) {
    enabled = false;
  }
  if (previewEditMode === enabled) {
    return;
  }
  previewEditMode = enabled;
  applyPreviewEditMode();
  syncEditModeButton();
  if (!enabled && state) {
    closeActiveModal();
    renderTablesEditor();
    queueRender();
  }
}

function syncEditModeButton(): void {
  for (const button of editModeButtons) {
    button.textContent = previewEditMode ? "Done" : "Edit";
    button.setAttribute("aria-pressed", previewEditMode ? "true" : "false");
    button.classList.toggle("primary", previewEditMode);
  }
}

function queueRender(): void {
  if (renderTimer) {
    window.clearTimeout(renderTimer);
  }
  renderTimer = window.setTimeout(() => {
    void renderAndValidate();
  }, 350);
}

async function renderAndValidate(): Promise<void> {
  if (!state) {
    return;
  }
  if (renderTimer) {
    window.clearTimeout(renderTimer);
    renderTimer = undefined;
  }
  clearDiagnostics();
  revokeAssetUrls();
  try {
    const bytes = await packageBytes();
    const doc = await openMcd(bytes);
    const validation = doc.validate();
    renderDiagnostics(validation);
    const blocks = validation.valid ? doc.blocks() : [];
    const markdown = validation.valid ? doc.markdown({ expandTables: true }) : state.markdown;
    await renderMarkdownPreview(markdown, blocks);
    setStatus(
      validation.valid ? "" : "Document has validation errors. Preview is rendered from the Markdown editor.",
    );
  } catch (error) {
    await renderMarkdownPreview(state.markdown);
    showError(error);
  }
}

function annotationPreviewItems(markdown: string): AnnotationPreviewItem[] {
  if (!state || state.annotations.length === 0) {
    return [];
  }

  const lineCount = markdownLineCount(markdown);
  const inlinePositions = inlineAnnotationPositions(markdown);
  const sortable = state.annotations.map((annotation, index) => {
    const inlinePosition = inlinePositions.get(annotation.id);
    const targetLine = Number(annotation.line);
    const line =
      inlinePosition?.line ??
      (Number.isInteger(targetLine) && targetLine > 0 ? targetLine : 1);
    return {
      id: annotation.id,
      annotation,
      line: Math.min(Math.max(1, line), lineCount),
      column: inlinePosition?.column ?? Number.MAX_SAFE_INTEGER,
      hasInlineMarker: Boolean(inlinePosition),
      manifestIndex: index,
    };
  });

  sortable.sort((left, right) => {
    return (
      left.line - right.line ||
      left.column - right.column ||
      left.manifestIndex - right.manifestIndex
    );
  });

  return sortable.map((item, index) => ({
    id: item.id,
    annotation: item.annotation,
    line: item.line,
    hasInlineMarker: item.hasInlineMarker,
    number: index + 1,
  }));
}

function inlineAnnotationPositions(markdown: string): Map<string, { line: number; column: number }> {
  const positions = new Map<string, { line: number; column: number }>();
  const lines = markdown.split(/\r\n|\r|\n/);
  const markerPattern = /\[\[annotation:([A-Za-z0-9][A-Za-z0-9_.-]*)\]\]/g;
  const bodyToId = new Map(state?.annotations.map((annotation) => [annotation.body, annotation.id]));
  const generatedPattern = /\(@annotation:\s*\[([^\]]*)\]\)/g;

  lines.forEach((line, lineIndex) => {
    markerPattern.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = markerPattern.exec(line)) !== null) {
      const id = match[1];
      if (id && !positions.has(id)) {
        positions.set(id, { line: lineIndex + 1, column: match.index + 1 });
      }
    }

    if (isStandaloneGeneratedAnnotationLine(line)) {
      return;
    }

    generatedPattern.lastIndex = 0;
    while ((match = generatedPattern.exec(line)) !== null) {
      const id = bodyToId.get(match[1] ?? "");
      if (id && !positions.has(id)) {
        positions.set(id, { line: lineIndex + 1, column: match.index + 1 });
      }
    }
  });

  return positions;
}

function annotatedPreviewMarkdown(markdown: string, items: AnnotationPreviewItem[]): string {
  const itemById = new Map(items.map((item) => [item.id, item]));
  const lineMarkers = new Map<number, AnnotationPreviewItem[]>();
  for (const item of items) {
    if (item.hasInlineMarker) {
      continue;
    }
    const markers = lineMarkers.get(item.line) ?? [];
    markers.push(item);
    lineMarkers.set(item.line, markers);
  }

  const lines = markdown.split(/\r\n|\r|\n/).map((line, index) => {
    if (isStandaloneGeneratedAnnotationLine(line)) {
      return "";
    }

    let withInlineMarkers = line.replace(
      /\[\[annotation:([A-Za-z0-9][A-Za-z0-9_.-]*)\]\]/g,
      (_raw, id: string) => {
        const item = itemById.get(id);
        return item ? annotationMarkerHtml(item) : "";
      },
    );
    withInlineMarkers = withInlineMarkers.replace(
      /\(@annotation:\s*\[([^\]]*)\]\)/g,
      (_raw, body: string) => {
        const item = items.find((candidate) => candidate.annotation.body === body);
        return item ? annotationMarkerHtml(item) : "";
      },
    );
    const markers = lineMarkers.get(index + 1);
    if (!markers || markers.length === 0) {
      return withInlineMarkers;
    }
    const markerHtml = markers
      .sort((left, right) => left.number - right.number)
      .map(annotationMarkerHtml)
      .join("");
    return withInlineMarkers.trim() ? `${withInlineMarkers} ${markerHtml}` : markerHtml;
  });

  return lines.join("\n");
}

function isStandaloneGeneratedAnnotationLine(line: string): boolean {
  return /^\s*\(@annotation:\s*\[[^\]]*\]\)\s*$/.test(line);
}

function annotationMarkerHtml(item: AnnotationPreviewItem): string {
  return `<sup id="mcd-annotation-ref-${escapeAttr(
    item.id,
  )}" class="mcd-annotation-marker"><a href="#mcd-annotation-${escapeAttr(
    item.id,
  )}" aria-label="Annotation ${item.number}">${item.number}</a></sup>`;
}

function annotationEndnotesNode(items: AnnotationPreviewItem[]): HTMLElement | undefined {
  if (items.length === 0) {
    return undefined;
  }

  const section = document.createElement("section");
  section.className = "mcd-annotations";
  section.setAttribute("aria-label", "Annotations");

  const heading = document.createElement("h2");
  heading.textContent = "Annotations";

  const list = document.createElement("ol");
  for (const item of items) {
    const entry = document.createElement("li");
    entry.id = `mcd-annotation-${item.id}`;

    const link = document.createElement("a");
    link.className = "mcd-annotation-backlink";
    link.href = `#mcd-annotation-ref-${item.id}`;
    link.setAttribute("aria-label", `Back to annotation ${item.number}`);

    const kind = document.createElement("span");
    kind.className = "mcd-annotation-kind";
    kind.textContent = item.annotation.kind;

    link.append(kind, document.createTextNode(`: ${item.annotation.body}`));
    entry.appendChild(link);
    list.appendChild(entry);
  }

  section.append(heading, list);
  return section;
}

async function renderMarkdownPreview(markdown: string, blocks: DocumentBlock[] = []): Promise<void> {
  const annotationItems = annotationPreviewItems(markdown);
  const rendered = marked.parse(annotatedPreviewMarkdown(markdown, annotationItems), {
    async: false,
  }) as string;
  const sanitized = DOMPurify.sanitize(rendered, {
    USE_PROFILES: { html: true, mathMl: true },
    ADD_ATTR: ["aria-label", "target"],
  });
  renderPagedPreview(sanitized, annotationItems);
  renderEmptyFirstHeadingPlaceholder(markdown, blocks);
  enhancePreviewDom();
  await rewritePackageImageSources();
  await waitForPreviewImages();
  repaginatePreview();
  enableInlinePreviewEditing(blocks);
}

function renderEmptyFirstHeadingPlaceholder(markdown: string, blocks: DocumentBlock[]): void {
  if (!state || markdown.trim() || blocks.length > 0) {
    return;
  }

  const pageBody = preview.querySelector<HTMLDivElement>(".preview-page-body");
  if (!pageBody) {
    return;
  }

  pageBody.querySelector(".empty-state")?.remove();
  const heading = document.createElement("h1");
  heading.id = EMPTY_FIRST_HEADING_ID;
  heading.className = "inline-empty-first-heading";
  heading.dataset.placeholder = "Title";
  pageBody.prepend(heading);
}

function enableInlinePreviewEditing(blocks: DocumentBlock[]): void {
  if (!state) {
    return;
  }
  inlineTextBindings = new WeakMap();
  inlineTableBindings = new WeakMap();
  previewBlockSources = new WeakMap();
  for (const marker of Array.from(
    preview.querySelectorAll<HTMLElement>(".mcd-annotation-marker, .mcd-citation-ref"),
  )) {
    marker.contentEditable = "false";
  }
  enableInlineTextEditing(blocks);
  enableInlineTableEditing(blocks);
  bindPreviewImageSources(blocks);
  applyTableHeaderPreferences(blocks);
  applyPreviewEditMode();
}

function enableInlineTextEditing(blocks: DocumentBlock[]): void {
  const candidates = editableTextCandidates();
  const boundElements = new Set<HTMLElement>();
  let cursor = 0;

  for (const block of blocks) {
    if (!isEditableTextBlock(block) || !block.source) {
      continue;
    }
    const blockText = normalizedEditableText(block.text);
    if (!blockText) {
      continue;
    }

    const matchIndex = candidates.findIndex(
      (candidate, index) => index >= cursor && candidateMatchesTextBlock(candidate, blockText),
    );
    if (matchIndex < 0) {
      continue;
    }

    const element = candidates[matchIndex];
    cursor = matchIndex + 1;
    bindInlineTextElement(element, block);
    boundElements.add(element);
  }

  for (const element of candidates) {
    if (boundElements.has(element) || inlineTextBindings.has(element)) {
      continue;
    }
    const emptyHeading = emptyFirstHeadingBlockForElement(element);
    bindInlineTextElement(element, emptyHeading);
  }
}

function editableTextCandidates(): HTMLElement[] {
  return Array.from(
    preview.querySelectorAll<HTMLElement>(
      ".preview-page-body h1, .preview-page-body h2, .preview-page-body h3, .preview-page-body h4, .preview-page-body h5, .preview-page-body h6, .preview-page-body p, .preview-page-body ul, .preview-page-body ol, .preview-page-body blockquote",
    ),
  ).filter((element) => {
    return !element.closest(".mcd-annotations, table, .mcd-math");
  });
}

function isEditableTextBlock(
  block: DocumentBlock,
): block is EditableTextBlock {
  return ["heading", "paragraph", "list", "quote"].includes(block.type);
}

function bindInlineTextElement(element: HTMLElement, block?: EditableTextBlock): void {
  element.tabIndex = 0;
  element.classList.add("inline-edit-target");
  inlineTextBindings.set(element, { block, source: block?.source });
  if (block?.source) {
    previewBlockSources.set(element, block.source);
  }

  element.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") {
      return;
    }
    const binding = inlineTextBindings.get(element);
    if (binding?.block?.type === "heading" && binding.source && previewEditMode) {
      splitHeadingInlineEdit(event, element, binding);
      return;
    }
    event.stopPropagation();
  });
  element.addEventListener("input", () => {
    if (!previewEditMode) {
      return;
    }
    const binding = inlineTextBindings.get(element);
    if (binding?.headingSplit) {
      updateMarkdownFromHeadingSplit(binding.headingSplit);
      return;
    }
    if (binding?.block) {
      updateMarkdownFromInlineText(element, binding);
    }
  });
}

function emptyFirstHeadingBlockForElement(element: HTMLElement): EditableTextBlock | undefined {
  if (element.id !== EMPTY_FIRST_HEADING_ID) {
    return undefined;
  }
  return {
    type: "heading",
    id: EMPTY_FIRST_HEADING_ID,
    level: 1,
    text: "",
    source: {
      startLine: 1,
      startColumn: 1,
      endLine: 1,
      endColumn: 1,
    },
  };
}

function splitHeadingInlineEdit(
  event: KeyboardEvent,
  element: HTMLElement,
  binding: InlineTextBinding,
): void {
  if (!binding.block || binding.block.type !== "heading" || !binding.source) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();

  const split = editableSelectionSplit(element);
  element.textContent = split.before || editableText(element);

  const continuation = document.createElement("p");
  continuation.tabIndex = 0;
  continuation.className = "inline-edit-target inline-editable inline-editable-heading-continuation";
  continuation.contentEditable = "true";
  continuation.spellcheck = true;
  continuation.setAttribute("aria-label", "Editing text");
  continuation.textContent = split.after;
  element.insertAdjacentElement("afterend", continuation);

  const headingSplit: InlineHeadingSplitBinding = {
    block: binding.block,
    source: binding.source,
    heading: element,
    continuation,
  };
  binding.headingSplit = headingSplit;
  inlineTextBindings.set(continuation, { headingSplit });

  continuation.addEventListener("keydown", (continuationEvent) => {
    if (continuationEvent.key === "Enter") {
      continuationEvent.stopPropagation();
    }
  });
  continuation.addEventListener("input", () => {
    if (previewEditMode) {
      updateMarkdownFromHeadingSplit(headingSplit);
    }
  });

  updateMarkdownFromHeadingSplit(headingSplit);
  focusEditableEnd(continuation);
}

function editableSelectionSplit(element: HTMLElement): { before: string; after: string } {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) {
    return { before: editableText(element), after: "" };
  }

  const range = selection.getRangeAt(0);
  if (!element.contains(range.commonAncestorContainer)) {
    return { before: editableText(element), after: "" };
  }

  const beforeRange = range.cloneRange();
  beforeRange.selectNodeContents(element);
  beforeRange.setEnd(range.startContainer, range.startOffset);

  const afterRange = range.cloneRange();
  afterRange.selectNodeContents(element);
  afterRange.setStart(range.endContainer, range.endOffset);

  return {
    before: normalizedRangeText(beforeRange),
    after: normalizedRangeText(afterRange),
  };
}

function normalizedRangeText(range: Range): string {
  return range.toString().replace(/\u00a0/g, " ").trim();
}

function focusEditableEnd(element: HTMLElement): void {
  element.focus();
  const range = document.createRange();
  range.selectNodeContents(element);
  range.collapse(false);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

function updateMarkdownFromHeadingSplit(binding: InlineHeadingSplitBinding): void {
  if (!state) {
    return;
  }
  const heading = editableText(binding.heading);
  const continuation = editableText(binding.continuation);
  const continuationLines = continuation
    .split(/\r\n|\r|\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const replacement = [
    `${"#".repeat(binding.block.level)} ${heading.trim()}`,
    ...continuationLines,
  ].join("\n");
  binding.source = replaceMarkdownSource(binding.source, replacement);
  markDirty({ render: false });
}

function updateMarkdownFromInlineText(element: HTMLElement, binding: InlineTextBinding): void {
  if (!state || !binding.block || !binding.source) {
    return;
  }

  const text = editableText(element);
  const replacement = markdownReplacementForInlineText(
    element,
    binding.block,
    binding.source,
    text,
  );
  binding.source = replaceMarkdownSource(binding.source, replacement);
  markDirty({ render: false });
}

function markdownReplacementForInlineText(
  element: HTMLElement,
  block: EditableTextBlock,
  source: SourceSpan,
  text: string,
): string {
  const lines = text
    .split(/\r\n|\r|\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const fallback = text.trim();

  if (block.type === "heading") {
    return `${"#".repeat(block.level)} ${fallback}`;
  }
  if (block.type === "quote") {
    return (lines.length > 0 ? lines : [fallback]).map((line) => `> ${line}`).join("\n");
  }
  if (block.type === "list") {
    return markdownListReplacement(element, source);
  }
  return fallback;
}

function markdownListReplacement(element: HTMLElement, source: SourceSpan): string {
  const itemTexts = Array.from(element.querySelectorAll<HTMLLIElement>("li"))
    .map((item) => editableText(item).trim())
    .filter(Boolean);
  if (itemTexts.length === 0) {
    return "";
  }
  const sourceLines = state?.markdown.split(/\r\n|\r|\n/).slice(source.startLine - 1, source.endLine) ?? [];
  const markers = sourceLines
    .map((line) => /^(\s*(?:[-*+]|\d+[.)])\s+)/.exec(line)?.[1])
    .filter((marker): marker is string => Boolean(marker));
  return itemTexts
    .map((item, index) => `${markers[index] ?? "- "}${item}`)
    .join("\n");
}

function replaceMarkdownSource(source: SourceSpan, replacement: string): SourceSpan {
  if (!state) {
    return source;
  }
  const lines = state.markdown.split(/\r\n|\r|\n/);
  const startIndex = Math.max(0, source.startLine - 1);
  const deleteCount = Math.max(1, source.endLine - source.startLine + 1);
  const existing = lines.slice(startIndex, startIndex + deleteCount).join("\n");
  if (existing === replacement) {
    return source;
  }
  recordHistoryCheckpoint({ coalesceKey: "preview-markdown" });
  const replacementLines = replacement.split("\n");
  lines.splice(startIndex, deleteCount, ...replacementLines);
  state.markdown = lines.join("\n");
  markdownEditor.value = state.markdown;
  return {
    ...source,
    endLine: source.startLine + replacementLines.length - 1,
    endColumn: replacementLines.at(-1)?.length ?? source.startColumn,
  };
}

function editableText(element: HTMLElement): string {
  const clone = element.cloneNode(true) as HTMLElement;
  for (const ignored of Array.from(
    clone.querySelectorAll(".mcd-annotation-marker, .mcd-citation-ref"),
  )) {
    ignored.remove();
  }
  return editableNodeText(clone, clone)
    .replace(/\u00a0/g, " ")
    .replace(/[ \t]+\n/g, "\n")
    .replace(/\n[ \t]+/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function editableNodeText(node: Node, root: Node): string {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.textContent ?? "";
  }
  if (!(node instanceof HTMLElement)) {
    return Array.from(node.childNodes)
      .map((child) => editableNodeText(child, root))
      .join("");
  }
  if (node.tagName === "BR") {
    return "\n";
  }

  const text = Array.from(node.childNodes)
    .map((child) => editableNodeText(child, root))
    .join("");
  if (node !== root && isEditableLineBreakElement(node)) {
    return `\n${text}\n`;
  }
  return text;
}

function isEditableLineBreakElement(element: HTMLElement): boolean {
  return [
    "ADDRESS",
    "ARTICLE",
    "ASIDE",
    "BLOCKQUOTE",
    "DD",
    "DIV",
    "DL",
    "DT",
    "FIGCAPTION",
    "FIGURE",
    "FOOTER",
    "H1",
    "H2",
    "H3",
    "H4",
    "H5",
    "H6",
    "HEADER",
    "LI",
    "MAIN",
    "NAV",
    "OL",
    "P",
    "PRE",
    "SECTION",
    "TR",
    "UL",
  ].includes(element.tagName);
}

function normalizedEditableText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

function candidateMatchesTextBlock(candidate: HTMLElement, blockText: string): boolean {
  const candidateText = normalizedEditableText(editableText(candidate));
  if (candidateText === blockText || candidateText.includes(blockText)) {
    return true;
  }

  const requiredWords = significantWords(blockText).slice(0, 8);
  if (requiredWords.length === 0) {
    return false;
  }
  const candidateWords = significantWords(candidateText);
  let cursor = 0;
  for (const word of requiredWords) {
    const index = candidateWords.indexOf(word, cursor);
    if (index < 0) {
      return false;
    }
    cursor = index + 1;
  }
  return true;
}

function significantWords(value: string): string[] {
  return value
    .toLowerCase()
    .match(/[a-z0-9]+/g)
    ?.filter((word) => word.length > 2) ?? [];
}

function applyPreviewEditMode(): void {
  preview.classList.toggle("is-edit-mode", previewEditMode);
  for (const element of Array.from(preview.querySelectorAll<HTMLElement>(".inline-edit-target"))) {
    const isEditable = previewEditMode && !element.closest(".mcd-annotations");
    element.contentEditable = isEditable ? "true" : "false";
    element.spellcheck = isEditable;
    element.classList.toggle("inline-editable", isEditable);
    element.setAttribute(
      "aria-label",
      isEditable
        ? inlineTableBindings.has(element)
          ? "Editing table cell"
          : "Editing text"
        : inlineTableBindings.has(element)
          ? "Table cell"
          : "Text block",
    );
  }
  renderInsertionGuides();
}

function enableInlineTableEditing(blocks: DocumentBlock[] = []): void {
  if (!state) {
    return;
  }

  const placements = tablePlacementsFromBlocks(blocks);
  if (placements.length === 0) {
    placements.push(...tablePlacementsFromMarkdown(state.markdown));
  }
  const previewTables = Array.from(
    preview.querySelectorAll<HTMLTableElement>(".preview-page-body table"),
  ).filter((table) => !table.closest(".mcd-annotations"));
  let tableCursor = 0;

  for (const placement of placements) {
    const tableState = state.tables.find((table) => table.manifest.id === placement.table);
    if (!tableState) {
      continue;
    }
    const columns = columnsForPlacement(tableState, placement);
    if (columns.length === 0) {
      continue;
    }

    const matchIndex = findPreviewTableForColumns(previewTables, tableCursor, columns);
    if (matchIndex < 0) {
      continue;
    }
    bindInlineTable(previewTables[matchIndex], tableState, columns);
    if (placement.source) {
      const tableElement = previewTables[matchIndex];
      previewBlockSources.set(tableElement, placement.source);
      const wrapper = tableElement.closest<HTMLElement>(".preview-table-wrap");
      if (wrapper) {
        previewBlockSources.set(wrapper, placement.source);
      }
    }
    tableCursor = matchIndex + 1;
  }
}

function tablePlacementsFromBlocks(blocks: DocumentBlock[]): TablePlacement[] {
  return blocks.flatMap((block) => {
    if (block.type !== "table_ref") {
      return [];
    }
    const placement = block.placement as {
      table?: unknown;
      view?: unknown;
      display?: unknown;
    };
    if (typeof placement.table !== "string") {
      return [];
    }
    return [
      {
        table: placement.table,
        view: typeof placement.view === "string" ? placement.view : undefined,
        display: placement.display === "chart" ? "chart" : "table",
        source: block.source,
      },
    ];
  });
}

function tablePlacementsFromMarkdown(markdown: string): TablePlacement[] {
  const placements: TablePlacement[] = [];
  const directivePattern = /(?:^|\n):::\s*table[^\n]*\n([\s\S]*?)\n:::/g;
  let match: RegExpExecArray | null;
  while ((match = directivePattern.exec(markdown)) !== null) {
    const fields = directiveFields(match[1] ?? "");
    const table = fields.get("table");
    if (!table) {
      continue;
    }
    placements.push({
      table,
      view: fields.get("view"),
      display: fields.get("display") === "chart" ? "chart" : "table",
    });
  }
  return placements;
}

function bindPreviewImageSources(blocks: DocumentBlock[]): void {
  const imageBlocks = blocks.filter(
    (block): block is Extract<DocumentBlock, { type: "image_ref" }> =>
      block.type === "image_ref" && Boolean(block.source),
  );
  if (imageBlocks.length === 0) {
    return;
  }

  const images = Array.from(
    preview.querySelectorAll<HTMLImageElement>(".preview-page-body img"),
  ).filter((image) => !image.closest(".mcd-annotations"));

  images.forEach((image, index) => {
    const source = imageBlocks[index]?.source;
    if (!source) {
      return;
    }
    previewBlockSources.set(image, source);
    const topLevel = previewTopLevelElement(image);
    if (topLevel) {
      previewBlockSources.set(topLevel, source);
    }
  });
}

function applyTableHeaderPreferences(blocks: DocumentBlock[]): void {
  if (!state) {
    return;
  }

  const placements = tablePlacementsFromBlocks(blocks);
  if (placements.length === 0) {
    placements.push(...tablePlacementsFromMarkdown(state.markdown));
  }

  const previewTables = Array.from(
    preview.querySelectorAll<HTMLTableElement>(".preview-page-body table"),
  ).filter((table) => !table.closest(".mcd-annotations"));
  let tableCursor = 0;

  for (const placement of placements) {
    const tableState = state.tables.find((table) => table.manifest.id === placement.table);
    if (!tableState) {
      continue;
    }
    const columns = columnsForPlacement(tableState, placement);
    if (columns.length === 0) {
      continue;
    }
    const matchIndex = findPreviewTableForColumns(previewTables, tableCursor, columns);
    if (matchIndex < 0) {
      continue;
    }
    const preferences = tableHeaderPreferences(tableState, placement);
    applyHeaderPreferencesToTable(previewTables[matchIndex], preferences);
    tableCursor = matchIndex + 1;
  }
}

function tableHeaderPreferences(
  table: EditableTable,
  placement: TablePlacement,
): { showColumnHeaders: boolean; showRowHeaders: boolean } {
  const view = placement.view ? table.views[placement.view] : undefined;
  return {
    showColumnHeaders: view?.style?.showColumnHeaders !== false,
    showRowHeaders: view?.style?.showRowHeaders === true,
  };
}

function applyHeaderPreferencesToTable(
  table: HTMLTableElement,
  preferences: { showColumnHeaders: boolean; showRowHeaders: boolean },
): void {
  if (!preferences.showRowHeaders) {
    removeReservedRowHeaderColumn(table);
  } else {
    convertReservedColumnToRowHeaders(table);
  }
  if (!preferences.showColumnHeaders) {
    table.querySelector("thead")?.remove();
  }
}

function removeReservedRowHeaderColumn(table: HTMLTableElement): void {
  table.querySelector<HTMLTableCellElement>("thead tr > :first-child")?.remove();
  Array.from(table.querySelectorAll<HTMLTableRowElement>("tbody tr")).forEach((row) => {
    const firstCell = row.querySelector<HTMLTableCellElement>("td, th");
    if (!firstCell) {
      return;
    }
    unbindInlineTableCell(firstCell);
    firstCell.remove();
  });
}

function convertReservedColumnToRowHeaders(table: HTMLTableElement): void {
  Array.from(table.querySelectorAll<HTMLTableRowElement>("tbody tr")).forEach((row, rowIndex) => {
    const firstCell = row.querySelector<HTMLTableCellElement>("td, th");
    if (!firstCell || firstCell.tagName === "TH") {
      return;
    }
    const header = document.createElement("th");
    header.scope = "row";
    for (const attribute of Array.from(firstCell.attributes)) {
      header.setAttribute(attribute.name, attribute.value);
    }
    header.innerHTML = firstCell.innerHTML;
    header.className = firstCell.className;
    header.tabIndex = firstCell.tabIndex;
    header.title = firstCell.title;
    const binding = inlineTableBindings.get(firstCell);
    if (binding) {
      bindInlineTableCell(header, binding, rowIndex);
    }
    firstCell.replaceWith(header);
  });
}

function previewTopLevelElement(element: HTMLElement): HTMLElement | undefined {
  let current: HTMLElement | null = element;
  while (current?.parentElement && !current.parentElement.classList.contains("preview-page-body")) {
    current = current.parentElement;
  }
  return current?.parentElement?.classList.contains("preview-page-body") ? current : undefined;
}

function directiveFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  for (const line of body.split(/\r\n|\r|\n/)) {
    const [key, ...rest] = line.split(":");
    if (!key || rest.length === 0) {
      continue;
    }
    fields.set(key.trim(), rest.join(":").trim());
  }
  return fields;
}

function columnsForPlacement(
  table: EditableTable,
  placement: TablePlacement,
): Array<TableViewColumn & { label: string; schema: TableColumn }> {
  const view = placement.view ? table.views[placement.view] : undefined;
  const schemaByName = new Map(table.schema.columns.map((column) => [column.name, column]));
  const requested =
    placement.display === "chart" && view?.chart
      ? chartColumns(view.chart)
      : (view?.columns ?? table.schema.columns);

  return requested.flatMap((column) => {
    const schema = schemaByName.get(column.name);
    if (!schema) {
      return [];
    }
    return [
      {
        ...column,
        label: column.label ?? schema.label ?? column.name,
        schema,
      },
    ];
  });
}

function chartColumns(chart: NonNullable<TableView["chart"]>): TableViewColumn[] {
  const seen = new Set<string>();
  const columns: TableViewColumn[] = [];
  for (const column of [chart.x, chart.y, chart.series, chart.grouping, chart.markLabels]) {
    if (!column?.column || seen.has(column.column)) {
      continue;
    }
    seen.add(column.column);
    columns.push({
      name: column.column,
      label: column.label,
      format: column.format,
      currency: column.currency,
      unit: column.unit,
      percent: column.percent,
    });
  }
  return columns;
}

function findPreviewTableForColumns(
  tables: HTMLTableElement[],
  startIndex: number,
  columns: Array<TableViewColumn & { label: string; schema: TableColumn }>,
): number {
  const expected = columns.map((column) => normalizedEditableText(column.label));
  for (let index = startIndex; index < tables.length; index += 1) {
    const headers = Array.from(tables[index].querySelectorAll("thead th")).map((header) =>
      normalizedEditableText(header.textContent ?? ""),
    );
    if (
      headers.length === expected.length &&
      headers.every((header, columnIndex) => header === expected[columnIndex])
    ) {
      return index;
    }
  }
  return -1;
}

function bindInlineTable(
  tableElement: HTMLTableElement,
  tableState: EditableTable,
  columns: Array<TableViewColumn & { label: string; schema: TableColumn }>,
): void {
  tableElement.classList.add("inline-editable-table");
  const rows = Array.from(tableElement.querySelectorAll<HTMLTableRowElement>("tbody tr"));
  rows.forEach((rowElement, rowIndex) => {
    const row = tableState.rows[rowIndex];
    if (!row) {
      return;
    }
    const cells = Array.from(rowElement.querySelectorAll<HTMLTableCellElement>("td"));
    cells.forEach((cell, columnIndex) => {
      const column = columns[columnIndex];
      if (!column) {
        return;
      }
      cell.tabIndex = 0;
      cell.classList.add("inline-edit-target");
      cell.title = `${tableState.manifest.id} ${column.name} row ${rowIndex + 1}`;
      bindInlineTableCell(cell, { row, column }, rowIndex);
    });
  });
}

function bindInlineTableCell(
  cell: HTMLTableCellElement,
  binding: InlineTableBinding,
  rowIndex: number,
): void {
  inlineTableBindings.set(cell, binding);
  cell.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") {
      return;
    }
    event.stopPropagation();
  });
  cell.addEventListener("input", () => {
    if (!previewEditMode) {
      return;
    }
    const next = parseInlineTableValue(editableText(cell), binding.column);
    if ((binding.row[binding.column.name] ?? "") === next) {
      return;
    }
    recordHistoryCheckpoint({
      coalesceKey: `preview-table:${binding.column.name}:${rowIndex}`,
    });
    binding.row[binding.column.name] = next;
    markDirty({ render: false });
  });
}

function unbindInlineTableCell(cell: HTMLTableCellElement): void {
  inlineTableBindings.delete(cell);
  cell.classList.remove("inline-edit-target", "inline-editable");
  cell.contentEditable = "false";
  cell.removeAttribute("aria-label");
}

function parseInlineTableValue(
  value: string,
  column: TableViewColumn & { schema: TableColumn },
): string {
  let next = value.replace(/\u00a0/g, " ").trim();
  const affix = column.currency ?? column.unit;
  if (affix) {
    next = next
      .replace(new RegExp(`^${escapeRegExp(affix)}\\s+`, "i"), "")
      .replace(new RegExp(`\\s+${escapeRegExp(affix)}$`, "i"), "");
  }
  if (column.percent || column.format === "percent") {
    next = next.replace(/%$/, "").trim();
  }
  if (
    ["integer", "decimal"].includes(column.schema.type) ||
    ["number", "currency", "percent", "integer", "decimal"].includes(column.format ?? "")
  ) {
    next = next.replace(/,/g, "");
  }
  return next;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function renderInsertionGuides(): void {
  for (const layer of Array.from(preview.querySelectorAll(".mcd-insert-lines"))) {
    layer.remove();
  }
  if (!state || !previewEditMode) {
    return;
  }

  for (const body of Array.from(preview.querySelectorAll<HTMLDivElement>(".preview-page-body"))) {
    const height = body.clientHeight;
    if (height <= 0) {
      continue;
    }
    const lineHeight = previewInsertionLineHeight(body);
    const lineCount = Math.max(1, Math.floor(height / lineHeight));
    const layer = document.createElement("div");
    layer.className = "mcd-insert-lines";
    layer.setAttribute("aria-hidden", "false");

    for (let index = 0; index < lineCount; index += 1) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "mcd-insert-line";
      button.setAttribute("aria-label", `Insert content at line ${index + 1}`);
      button.style.top = `${index * lineHeight}px`;
      button.style.height = `${lineHeight}px`;
      button.innerHTML = `<span aria-hidden="true">+</span>`;
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        showInsertTypePopup({
          body,
          y: index * lineHeight + lineHeight / 2,
        });
      });
      layer.appendChild(button);
    }

    body.appendChild(layer);
  }
}

function previewInsertionLineHeight(body: HTMLElement): number {
  const styles = window.getComputedStyle(body);
  const lineHeight = Number.parseFloat(styles.lineHeight);
  if (Number.isFinite(lineHeight) && lineHeight > 0) {
    return Math.max(20, lineHeight);
  }
  const fontSize = Number.parseFloat(styles.fontSize);
  return Math.max(20, (Number.isFinite(fontSize) ? fontSize : 16) * 1.45);
}

function showInsertTypePopup(target: InsertLineTarget): void {
  const line = markdownInsertionLine(target);
  showModal(`
    <div class="mcd-popup">
      <div class="mcd-popup-header">
        <div class="mcd-popup-title">Add content</div>
        <button class="mcd-popup-close" type="button" data-action="close" aria-label="Close">&times;</button>
      </div>
      <div class="mcd-popup-actions">
        <button class="primary" type="button" data-action="table">Table</button>
        <button type="button" data-action="image">Image</button>
      </div>
    </div>
  `);
  activeModal
    ?.querySelector<HTMLButtonElement>('[data-action="table"]')
    ?.addEventListener("click", () => showTableSizePopup(line));
  activeModal
    ?.querySelector<HTMLButtonElement>('[data-action="image"]')
    ?.addEventListener("click", () => showImagePopup(line));
}

function showTableSizePopup(insertLine: number): void {
  showModal(`
    <form class="mcd-popup" id="tableCreateForm">
      <div class="mcd-popup-header">
        <div class="mcd-popup-title">Create table</div>
        <button class="mcd-popup-close" type="button" data-action="close" aria-label="Close">&times;</button>
      </div>
      <div class="mcd-popup-field-row">
        <div class="field">
          <label for="tableColumnCount">Columns</label>
          <input id="tableColumnCount" name="columns" type="number" min="1" max="24" step="1" value="3" required />
        </div>
        <label class="mcd-popup-check">
          <input name="columnHeaders" type="checkbox" checked />
          <span>with column headers</span>
        </label>
      </div>
      <div class="mcd-popup-field-row">
        <div class="field">
          <label for="tableRowCount">Rows</label>
          <input id="tableRowCount" name="rows" type="number" min="1" max="200" step="1" value="3" required />
        </div>
        <label class="mcd-popup-check">
          <input name="rowHeaders" type="checkbox" checked />
          <span>with row headers</span>
        </label>
      </div>
      <div class="mcd-popup-footer">
        <button class="primary" type="submit">Create</button>
      </div>
    </form>
  `);
  const form = activeModal?.querySelector<HTMLFormElement>("#tableCreateForm");
  form?.addEventListener("submit", (event) => {
    event.preventDefault();
    const data = new FormData(form);
    const columns = quantityFromForm(data.get("columns"), 1, 24, 3);
    const rows = quantityFromForm(data.get("rows"), 1, 200, 3);
    createTableAtLine(insertLine, columns, rows, {
      showColumnHeaders: data.has("columnHeaders"),
      showRowHeaders: data.has("rowHeaders"),
    });
    closeActiveModal();
  });
  activeModal?.querySelector<HTMLInputElement>("#tableColumnCount")?.focus();
}

function showImagePopup(insertLine: number): void {
  showModal(`
    <form class="mcd-popup" id="imageCreateForm">
      <div class="mcd-popup-header">
        <div class="mcd-popup-title">Create image</div>
        <button class="mcd-popup-close" type="button" data-action="close" aria-label="Close">&times;</button>
      </div>
      <div class="field">
        <label for="imageFileInput">Image</label>
        <input id="imageFileInput" name="image" type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml" required />
      </div>
      <div class="field">
        <label for="imageAltInput">Alt text</label>
        <input id="imageAltInput" name="alt" type="text" required />
      </div>
      <div class="mcd-popup-footer">
        <button class="primary" type="submit">Create</button>
      </div>
    </form>
  `);
  const form = activeModal?.querySelector<HTMLFormElement>("#imageCreateForm");
  const fileInput = activeModal?.querySelector<HTMLInputElement>("#imageFileInput");
  const altInput = activeModal?.querySelector<HTMLInputElement>("#imageAltInput");
  fileInput?.addEventListener("change", () => {
    const file = fileInput.files?.[0];
    if (file && altInput && !altInput.value.trim()) {
      altInput.value = file.name.replace(/\.[^.]+$/, "").replace(/[-_]+/g, " ");
    }
  });
  form?.addEventListener("submit", (event) => {
    event.preventDefault();
    const file = fileInput?.files?.[0];
    const alt = altInput?.value.trim() ?? "";
    if (!file || !alt) {
      return;
    }
    void createImageAtLine(insertLine, file, alt).then(() => closeActiveModal());
  });
  fileInput?.focus();
}

function showModal(html: string): void {
  closeActiveModal();
  const backdrop = document.createElement("div");
  backdrop.className = "mcd-popup-backdrop";
  backdrop.innerHTML = html;
  backdrop.addEventListener("click", (event) => {
    if (event.target === backdrop) {
      closeActiveModal();
    }
  });
  backdrop.querySelector('[data-action="close"]')?.addEventListener("click", closeActiveModal);
  activeModal = backdrop;
  document.body.appendChild(backdrop);
}

function closeActiveModal(): void {
  activeModal?.remove();
  activeModal = undefined;
}

function quantityFromForm(value: FormDataEntryValue | null, min: number, max: number, fallback: number): number {
  const parsed = Number(value);
  if (!Number.isInteger(parsed)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, parsed));
}

function markdownInsertionLine(target: InsertLineTarget): number {
  if (!state) {
    return 1;
  }
  const elements = Array.from(target.body.children).filter(
    (child): child is HTMLElement =>
      child instanceof HTMLElement &&
      !child.classList.contains("mcd-insert-lines") &&
      !child.classList.contains("empty-state"),
  );
  if (elements.length === 0) {
    return 1;
  }

  const bodyTop = target.body.getBoundingClientRect().top;
  let previousSource: SourceSpan | undefined;
  let nextSource: SourceSpan | undefined;

  for (const element of elements) {
    const rect = element.getBoundingClientRect();
    const top = rect.top - bodyTop;
    const bottom = rect.bottom - bodyTop;
    const source = sourceForPreviewElement(element);
    if (target.y < top) {
      nextSource = source;
      break;
    }
    if (target.y <= bottom) {
      if (target.y < top + (bottom - top) / 2) {
        nextSource = source;
      } else {
        previousSource = source;
      }
      break;
    }
    previousSource = source ?? previousSource;
  }

  if (nextSource) {
    return Math.max(1, nextSource.startLine);
  }
  if (previousSource) {
    return previousSource.endLine + 1;
  }
  return markdownLineCount(state.markdown) + 1;
}

function sourceForPreviewElement(element: HTMLElement): SourceSpan | undefined {
  const direct =
    previewBlockSources.get(element) ??
    inlineTextBindings.get(element)?.source;
  if (direct) {
    return direct;
  }

  const nested = element.querySelector<HTMLElement>(".inline-edit-target, table, img");
  if (!nested) {
    return undefined;
  }
  return previewBlockSources.get(nested) ?? inlineTextBindings.get(nested)?.source;
}

function createTableAtLine(
  insertLine: number,
  columnCount: number,
  rowCount: number,
  preferences: { showColumnHeaders: boolean; showRowHeaders: boolean },
): void {
  if (!state) {
    return;
  }
  recordHistoryCheckpoint();
  const id = nextTableId(state);
  const viewPath = `tables/${id}.view.json`;
  const entry: TableManifestEntry = {
    id,
    data: `tables/${id}.csv`,
    schema: `tables/${id}.schema.json`,
    views: {
      default: viewPath,
    },
  };
  const schema: TableSchema = {
    id,
    columns: [
      {
        name: RESERVED_ROW_HEADER_COLUMN,
        type: "string",
        label: "Rows",
        nullable: false,
      },
      ...Array.from({ length: columnCount }, (_, index) => ({
        name: `column_${index + 1}`,
        type: "string",
        label: `Column ${index + 1}`,
        nullable: true,
      })),
    ],
  };
  const view: TableView = {
    id: "default",
    table: id,
    display: "table",
    columns: schema.columns.map((column) => ({
      name: column.name,
      label: column.label,
    })),
    style: {
      showColumnHeaders: preferences.showColumnHeaders,
      showRowHeaders: preferences.showRowHeaders,
    },
  };
  const rows = Array.from({ length: rowCount }, (_unused, rowIndex) =>
    Object.fromEntries(
      schema.columns.map((column) => [
        column.name,
        column.name === RESERVED_ROW_HEADER_COLUMN ? String(rowIndex + 1) : "",
      ]),
    ),
  );
  const table: EditableTable = {
    manifest: entry,
    schema,
    views: {
      default: view,
    },
    rows,
  };

  state.manifest.tables ??= [];
  state.manifest.tables.push(entry);
  state.tables.push(table);
  state.zip.file(entry.schema, `${JSON.stringify(schema, null, 2)}\n`);
  state.zip.file(viewPath, `${JSON.stringify(view, null, 2)}\n`);
  state.zip.file(entry.data, tableToCsv(table));
  insertMarkdownBlockAtLine(insertLine, `:::table\ntable: ${id}\nview: default\n:::`);
  renderTablesEditor();
  markDirty();
  setStatus(`Created table '${id}'.`);
}

async function createImageAtLine(insertLine: number, file: File, alt: string): Promise<void> {
  if (!state) {
    return;
  }
  const mediaType = imageMediaType(file);
  if (!mediaType) {
    setStatus("Unsupported image type.");
    return;
  }

  recordHistoryCheckpoint();
  const id = nextImageId(state, file.name);
  const extension = imageExtension(file, mediaType);
  const assetPath = `assets/${id}.${extension}`;
  const metadataPath = `images/${id}.image.json`;
  const metadata = {
    id,
    asset: assetPath,
    mediaType,
    role: "photo",
    alt,
  };

  state.manifest.images ??= [];
  state.manifest.images.push({ id, metadata: metadataPath });
  state.manifest.assets ??= [];
  state.manifest.assets.push({ id, path: assetPath });
  state.zip.file(assetPath, new Uint8Array(await file.arrayBuffer()));
  state.zip.file(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`);
  insertMarkdownBlockAtLine(insertLine, `:::image\nimage: ${id}\nalt: ${alt}\n:::`);
  markDirty();
  setStatus(`Created image '${id}'.`);
}

function insertMarkdownBlockAtLine(insertLine: number, block: string): void {
  if (!state) {
    return;
  }
  const lines = state.markdown.split(/\r\n|\r|\n/);
  const hasContent = state.markdown.trim().length > 0;
  if (!hasContent) {
    state.markdown = block;
    markdownEditor.value = state.markdown;
    return;
  }

  const index = Math.min(Math.max(0, insertLine - 1), lines.length);
  const before = lines.slice(0, index).join("\n").trimEnd();
  const after = lines.slice(index).join("\n").trimStart();
  state.markdown = [before, block, after].filter(Boolean).join("\n\n");
  markdownEditor.value = state.markdown;
}

function nextTableId(packageState: PackageState): string {
  const existing = new Set((packageState.manifest.tables ?? []).map((table) => table.id));
  for (let index = 1; ; index += 1) {
    const id = `table-${String(index).padStart(4, "0")}`;
    if (!existing.has(id)) {
      return id;
    }
  }
}

function nextImageId(packageState: PackageState, fileName: string): string {
  const existing = new Set((packageState.manifest.images ?? []).map((image) => image.id));
  const base = sanitizeId(fileName.replace(/\.[^.]+$/, "") || "image");
  for (let index = 1; ; index += 1) {
    const id = index === 1 ? base : `${base}-${index}`;
    if (!existing.has(id)) {
      return id;
    }
  }
}

function imageMediaType(file: File): string | undefined {
  const allowed = new Set(["image/svg+xml", "image/png", "image/jpeg", "image/webp", "image/gif"]);
  if (allowed.has(file.type)) {
    return file.type;
  }
  const extension = file.name.toLowerCase().split(".").at(-1);
  if (extension === "svg") return "image/svg+xml";
  if (extension === "png") return "image/png";
  if (extension === "jpg" || extension === "jpeg") return "image/jpeg";
  if (extension === "webp") return "image/webp";
  if (extension === "gif") return "image/gif";
  return undefined;
}

function imageExtension(file: File, mediaType: string): string {
  const extension = file.name.toLowerCase().split(".").at(-1);
  if (extension && ["svg", "png", "jpg", "jpeg", "webp", "gif"].includes(extension)) {
    return extension === "jpg" ? "jpeg" : extension;
  }
  if (mediaType === "image/svg+xml") return "svg";
  return mediaType.replace("image/", "").replace("jpeg", "jpeg");
}

function renderPagedPreview(html: string, annotationItems: AnnotationPreviewItem[] = []): void {
  const template = document.createElement("template");
  template.innerHTML = html;
  const nodes = Array.from(template.content.childNodes).filter((node) => {
    return node.nodeType !== Node.TEXT_NODE || Boolean(node.textContent?.trim());
  });
  const annotationsNode = annotationEndnotesNode(annotationItems);
  if (annotationsNode) {
    nodes.push(annotationsNode);
  }

  preview.innerHTML = "";
  preview.classList.add("is-paged");

  const pageNumber = paginateNodes(nodes);
  if (nodes.length === 0) {
    const pageBody = preview.querySelector<HTMLDivElement>(".preview-page-body");
    if (pageBody) {
      pageBody.innerHTML = `<div class="empty-state">Document has no previewable content.</div>`;
    }
  }

  updatePageMapMetadata(pageNumber, annotationPreviewPageNumber());
}

function appendPreviewPage(pageNumber: number): { page: HTMLElement; body: HTMLDivElement } {
  const page = document.createElement("section");
  page.className = "preview-page";
  page.setAttribute("aria-label", `Page ${pageNumber}`);
  page.dataset.pageNumber = String(pageNumber);

  const body = document.createElement("div");
  body.className = "preview-page-body";

  const footer = document.createElement("footer");
  footer.className = "preview-page-number";
  footer.textContent = `Page ${pageNumber}`;

  page.append(body, footer);
  preview.appendChild(page);
  return { page, body };
}

function repaginatePreview(): void {
  const pages = Array.from(preview.querySelectorAll<HTMLElement>(".preview-page"));
  if (pages.length === 0) {
    return;
  }

  const nodes = pages.flatMap((page) =>
    Array.from(page.querySelector<HTMLDivElement>(".preview-page-body")?.childNodes ?? []),
  );
  preview.innerHTML = "";

  const pageNumber = paginateNodes(nodes);
  updatePageMapMetadata(pageNumber, annotationPreviewPageNumber());
}

function paginateNodes(nodes: Node[]): number {
  let cursor: PreviewPageCursor = {
    pageNumber: 1,
    page: appendPreviewPage(1),
  };

  for (const node of nodes) {
    cursor = appendNodeToPreviewPage(node, cursor);
  }

  return cursor.pageNumber;
}

interface PreviewPageCursor {
  pageNumber: number;
  page: { page: HTMLElement; body: HTMLDivElement };
}

function appendNodeToPreviewPage(node: Node, cursor: PreviewPageCursor): PreviewPageCursor {
  if (isForcedPreviewPageBreak(node) && cursor.page.body.childNodes.length > 0) {
    cursor = nextPreviewPage(cursor);
  }

  cursor.page.body.appendChild(node);
  if (!isPreviewPageOverflowing(cursor.page.body)) {
    return cursor;
  }

  const heading = previousHeadingForOverflowingNode(cursor.page.body, node);
  if (heading) {
    cursor.page.body.removeChild(node);
    cursor.page.body.removeChild(heading);
    cursor = appendNodeToPreviewPage(heading, nextPreviewPage(cursor));
    return appendNodeToPreviewPage(node, cursor);
  }

  if (cursor.page.body.childNodes.length > 1) {
    cursor.page.body.removeChild(node);
    return appendNodeToPreviewPage(node, nextPreviewPage(cursor));
  }

  cursor.page.page.classList.add("is-oversized");
  return cursor;
}

function nextPreviewPage(cursor: PreviewPageCursor): PreviewPageCursor {
  const pageNumber = cursor.pageNumber + 1;
  return {
    pageNumber,
    page: appendPreviewPage(pageNumber),
  };
}

function isForcedPreviewPageBreak(node: Node): boolean {
  return node instanceof HTMLElement && node.classList.contains("mcd-annotations");
}

function isPreviewPageOverflowing(body: HTMLDivElement): boolean {
  return body.scrollHeight > body.clientHeight + 1;
}

function splitParagraphElementToFit(
  body: HTMLDivElement,
  element: HTMLElement,
): HTMLElement | undefined {
  if (element.tagName !== "P") {
    return undefined;
  }

  const original = element.cloneNode(true) as HTMLElement;
  const breakpoints = textBreakOffsets(original.textContent ?? "");
  if (breakpoints.length === 0) {
    return undefined;
  }

  let low = 1;
  let high = breakpoints.length;
  let best = 0;

  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const prefix = cloneElementTextRange(original, 0, breakpoints[mid - 1]);
    replaceElementChildren(element, prefix);
    if (isPreviewPageOverflowing(body)) {
      high = mid - 1;
    } else {
      best = mid;
      low = mid + 1;
    }
  }

  if (best === 0) {
    replaceElementChildren(element, original);
    return undefined;
  }

  const splitOffset = breakpoints[best - 1];
  const prefix = cloneElementTextRange(original, 0, splitOffset);
  const remainder = cloneElementTextRange(original, splitOffset, textLength(original));
  trimEdgeText(prefix, "end");
  trimEdgeText(remainder, "start");

  if (!remainder.textContent?.trim()) {
    replaceElementChildren(element, original);
    return undefined;
  }

  replaceElementChildren(element, prefix);
  return remainder;
}

function textBreakOffsets(text: string): number[] {
  const offsets: number[] = [];
  let cursor = 0;
  for (const segment of text.match(/\S+\s*/g) ?? []) {
    cursor += segment.length;
    if (cursor < text.length) {
      offsets.push(cursor);
    }
  }
  return offsets;
}

function cloneElementTextRange(source: HTMLElement, start: number, end: number): HTMLElement {
  const clone = source.cloneNode(false) as HTMLElement;
  const position = { value: 0 };
  for (const child of Array.from(source.childNodes)) {
    const clonedChild = cloneTextRange(child, start, end, position);
    if (clonedChild) {
      clone.appendChild(clonedChild);
    }
  }
  return clone;
}

function cloneTextRange(
  node: Node,
  start: number,
  end: number,
  position: { value: number },
): Node | undefined {
  if (node.nodeType === Node.TEXT_NODE) {
    const text = node.textContent ?? "";
    const nodeStart = position.value;
    const nodeEnd = nodeStart + text.length;
    position.value = nodeEnd;
    const sliceStart = Math.max(start, nodeStart);
    const sliceEnd = Math.min(end, nodeEnd);
    if (sliceStart >= sliceEnd) {
      return undefined;
    }
    return document.createTextNode(text.slice(sliceStart - nodeStart, sliceEnd - nodeStart));
  }

  if (!(node instanceof HTMLElement)) {
    return undefined;
  }

  const clone = node.cloneNode(false) as HTMLElement;
  for (const child of Array.from(node.childNodes)) {
    const clonedChild = cloneTextRange(child, start, end, position);
    if (clonedChild) {
      clone.appendChild(clonedChild);
    }
  }
  return clone.childNodes.length > 0 ? clone : undefined;
}

function textLength(root: Node): number {
  let length = 0;
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    length += walker.currentNode.textContent?.length ?? 0;
  }
  return length;
}

function replaceElementChildren(target: HTMLElement, source: HTMLElement): void {
  target.replaceChildren(...Array.from(source.childNodes).map((node) => node.cloneNode(true)));
}

function trimEdgeText(root: HTMLElement, edge: "start" | "end"): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  while (walker.nextNode()) {
    textNodes.push(walker.currentNode as Text);
  }
  const node = edge === "start" ? textNodes[0] : textNodes.at(-1);
  if (!node) {
    return;
  }
  node.textContent = edge === "start" ? node.data.trimStart() : node.data.trimEnd();
}

function previousHeadingForOverflowingNode(
  body: HTMLDivElement,
  node: Node,
): HTMLElement | undefined {
  if (!(node instanceof HTMLElement) || body.childNodes.length <= 2) {
    return undefined;
  }

  const previous = node.previousElementSibling;
  if (!previous || !/^H[1-6]$/.test(previous.tagName)) {
    return undefined;
  }

  return previous as HTMLElement;
}

function enhancePreviewDom(): void {
  for (const link of Array.from(preview.querySelectorAll<HTMLAnchorElement>("a[href]"))) {
    const href = link.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href)) {
      link.target = "_blank";
      link.rel = "noopener noreferrer";
    }
  }
  enhanceCitationLinks();

  for (const table of Array.from(preview.querySelectorAll<HTMLTableElement>("table"))) {
    if (table.parentElement?.classList.contains("preview-table-wrap")) {
      continue;
    }
    const wrapper = document.createElement("div");
    wrapper.className = "preview-table-wrap";
    wrapper.setAttribute("role", "region");
    wrapper.setAttribute("tabindex", "0");
    wrapper.setAttribute("aria-label", "Scrollable table");
    table.replaceWith(wrapper);
    wrapper.appendChild(table);
  }

  for (const math of Array.from(preview.querySelectorAll<HTMLElement>(".mcd-math"))) {
    math.setAttribute("tabindex", "0");
  }
}

function enhanceCitationLinks(): void {
  for (const link of Array.from(preview.querySelectorAll<HTMLAnchorElement>("a[href]"))) {
    if (link.closest("sup, pre, code, .mcd-annotation-marker, .mcd-citation-ref")) {
      continue;
    }
    const citation = numericCitationLabel(link);
    if (!citation) {
      continue;
    }

    link.textContent = `[${citation}]`;
    link.setAttribute("aria-label", `Reference ${citation}`);
    const marker = document.createElement("sup");
    marker.className = "mcd-citation-ref";
    link.replaceWith(marker);
    marker.appendChild(link);
  }
}

function numericCitationLabel(link: HTMLAnchorElement): string | undefined {
  const text = (link.textContent ?? "").trim();
  const bracketed = /^\[(\d+)\]$/.exec(text);
  if (bracketed?.[1]) {
    return bracketed[1];
  }

  const href = link.getAttribute("href") ?? "";
  const bare = /^(\d+)$/.exec(text);
  if (bare?.[1] && /(?:^|[#/?=&-])cite(?:_|-)?note(?:$|[#/?=&-])/i.test(href)) {
    return bare[1];
  }

  return undefined;
}

async function waitForPreviewImages(): Promise<void> {
  const images = Array.from(preview.querySelectorAll<HTMLImageElement>("img"));
  await Promise.all(
    images.map((image) => {
      if (image.complete) {
        return Promise.resolve();
      }
      return new Promise<void>((resolve) => {
        image.addEventListener("load", () => resolve(), { once: true });
        image.addEventListener("error", () => resolve(), { once: true });
      });
    }),
  );
}

function annotationPreviewPageNumber(): number | undefined {
  const annotationPage = preview.querySelector<HTMLElement>(".mcd-annotations")?.closest<HTMLElement>(
    ".preview-page",
  );
  if (!annotationPage) {
    return undefined;
  }
  const pageNumber = Number(annotationPage.dataset.pageNumber);
  if (!Number.isInteger(pageNumber) || pageNumber < 1) {
    return undefined;
  }
  annotationPage.setAttribute("aria-label", "Annotations");
  annotationPage.querySelector(".preview-page-number")?.replaceChildren("Annotations");
  return pageNumber;
}

function updatePageMapMetadata(pageCount: number, annotationPageNumber?: number): void {
  if (!state) {
    return;
  }

  const previousPages = state.pageMap?.pages ?? [];
  const pageMapPath = state.pageMapPath ?? state.manifest.layout?.pageMap ?? "layout/page-map.json";
  state.pageMapPath = pageMapPath;
  state.manifest.layout = {
    ...(state.manifest.layout ?? {}),
    pageMap: pageMapPath,
  };
  state.pageMap = {
    pages: Array.from({ length: pageCount }, (_, index) => {
      const number = index + 1;
      const previous = previousPages[index];
      return {
        number,
        label: number === annotationPageNumber ? "Annotations" : (previous?.label ?? `Page ${number}`),
        ...(previous?.sourceRefs ? { sourceRefs: previous.sourceRefs } : {}),
        ...(previous?.assets ? { assets: previous.assets } : {}),
        ...(previous?.rendered ? { rendered: previous.rendered } : {}),
      };
    }),
  };
}

async function rewritePackageImageSources(): Promise<void> {
  if (!state) {
    return;
  }
  const images = Array.from(preview.querySelectorAll<HTMLImageElement>("img"));
  for (const image of images) {
    const source = image.getAttribute("src");
    if (!source || source.includes(":") || source.startsWith("/")) {
      continue;
    }
    const file = state.zip.file(source);
    if (!file) {
      continue;
    }
    const blob = await file.async("blob");
    const objectUrl = URL.createObjectURL(blob);
    assetUrls.push(objectUrl);
    image.src = objectUrl;
  }
}

function renderDiagnostics(validation: ValidationResult): void {
  clearDiagnostics();
  for (const diagnostic of validation.diagnostics) {
    diagnosticsEl.appendChild(diagnosticNode(diagnostic));
  }
}

function diagnosticNode(diagnostic: Diagnostic): HTMLDivElement {
  const node = document.createElement("div");
  node.className = "diagnostic";
  const source = diagnostic.source ? ` (${diagnostic.source})` : "";
  node.textContent = `${diagnostic.code}${source}: ${diagnostic.message}`;
  return node;
}

async function packageBytes(): Promise<Uint8Array> {
  if (!state) {
    throw new Error("No document is loaded.");
  }
  applyStateToZip(state);
  return state.zip.generateAsync({
    type: "uint8array",
    compression: "DEFLATE",
    mimeType: MCD_MIMETYPE,
  });
}

function applyStateToZip(packageState: PackageState): void {
  packageState.zip.file("mimetype", `${MCD_MIMETYPE}\n`, { compression: "STORE" });
  packageState.zip.file(packageState.manifest.entrypoint, packageState.markdown);
  packageState.manifest.annotations = packageState.annotations.map((annotation) => ({
    id: annotation.id,
    metadata: annotation.metadata,
  }));
  for (const path of packageState.removedAnnotationPaths) {
    packageState.zip.remove(path);
  }

  for (const table of packageState.tables) {
    packageState.zip.file(table.manifest.schema, `${JSON.stringify(table.schema, null, 2)}\n`);
    packageState.zip.file(table.manifest.data, tableToCsv(table));
    for (const [viewId, view] of Object.entries(table.views)) {
      const path = table.manifest.views?.[viewId];
      if (path) {
        packageState.zip.file(path, `${JSON.stringify(view, null, 2)}\n`);
      }
    }
  }
  for (const annotation of packageState.annotations) {
    packageState.zip.file(
      annotation.metadata,
      `${JSON.stringify(annotationToJson(annotation), null, 2)}\n`,
    );
  }
  if (packageState.pageMapPath && packageState.pageMap) {
    packageState.zip.file(
      packageState.pageMapPath,
      `${JSON.stringify(packageState.pageMap, null, 2)}\n`,
    );
  }
  packageState.zip.file("manifest.json", `${JSON.stringify(packageState.manifest, null, 2)}\n`);
}

function tableToCsv(table: EditableTable): string {
  const fields = table.schema.columns.map((column) => column.name);
  const data = table.rows.map((row) => fields.map((field) => row[field] ?? ""));
  return `${Papa.unparse({ fields, data }, { newline: "\n" })}\n`;
}

function renderMath(tex: string, displayMode: boolean): string {
  const expression = tex.trim();
  const tag = displayMode ? "div" : "span";
  const className = displayMode ? "mcd-math" : "mcd-inline-math";
  if (!expression) {
    return displayMode ? "" : "$$";
  }

  try {
    const rendered = katex.renderToString(expression, {
      displayMode,
      output: "htmlAndMathml",
      throwOnError: false,
      trust: false,
      strict: "warn",
      maxSize: 20,
      maxExpand: 1000,
    });
    return `<${tag} class="${className}" data-mcd-math="${displayMode ? "display" : "inline"}">${rendered}</${tag}>`;
  } catch (error) {
    const message = error instanceof Error ? error.message : "Invalid math expression.";
    if (displayMode) {
      return `<pre class="mcd-math mcd-math-fallback" data-mcd-math="display" data-mcd-math-error="${escapeAttr(
        message,
      )}"><code>${escapeHtml(expression)}</code></pre>`;
    }
    return `<code class="mcd-inline-math mcd-math-fallback" data-mcd-math="inline" title="${escapeAttr(
      message,
    )}">${escapeHtml(expression)}</code>`;
  }
}

function annotationToJson(annotation: EditableAnnotation): Record<string, unknown> {
  const line = Number(annotation.line);
  const output: Record<string, unknown> = {
    id: annotation.id,
    target:
      state && Number.isInteger(line) && line > 0
        ? sourceLineTarget(state.manifest.entrypoint, line)
        : (JSON.parse(annotation.targetText) as unknown),
    kind: annotation.kind,
    status: annotation.status,
    body: annotation.body,
  };
  if (annotation.author.trim()) {
    output.author = annotation.author.trim();
  }
  if (annotation.created.trim()) {
    output.created = annotation.created.trim();
  }
  const labels = annotation.labels
    .split(",")
    .map((label) => label.trim())
    .filter(Boolean);
  if (labels.length > 0) {
    output.labels = [...new Set(labels)];
  }
  return output;
}

async function saveAnnotationLocally(annotation: EditableAnnotation): Promise<void> {
  if (!state) {
    return;
  }
  try {
    applyStateToZip(state);
    locallySavedAnnotationIds.add(annotation.id);
    state.dirty = true;
    fileNameEl.textContent = `${state.fileName} (edited)`;
    await renderAndValidate();
    renderAnnotationsEditor();
    setStatus(
      `Saved annotation '${annotation.id}' locally in this browser session. Save .mcd to write the full file.`,
    );
  } catch (error) {
    showError(error);
  }
}

async function saveDocument(): Promise<void> {
  if (!state) {
    return;
  }
  try {
    await renderAndValidate();
    const bytes = await packageBytes();
    const doc = await openMcd(bytes);
    const validation = doc.validate();
    renderDiagnostics(validation);
    if (!validation.valid) {
      setStatus("Fix validation errors before saving.");
      return;
    }
    const blobBytes = new ArrayBuffer(bytes.byteLength);
    new Uint8Array(blobBytes).set(bytes);
    const blob = new Blob([blobBytes], { type: MCD_MIMETYPE });
    const suggestedName = outputFileName(state.fileName, state.plainMarkdownInput);
    const picker = (window as SaveFilePickerWindow).showSaveFilePicker;
    if (picker) {
      const handle = await picker({
        suggestedName,
        types: [
          {
            description: "MCD document",
            accept: { [MCD_MIMETYPE]: [".mcd"] },
          },
        ],
      });
      const writable = await handle.createWritable();
      await writable.write(blob);
      await writable.close();
    } else {
      const link = document.createElement("a");
      link.href = URL.createObjectURL(blob);
      link.download = suggestedName;
      link.click();
      URL.revokeObjectURL(link.href);
    }
    state.dirty = false;
    const snapshot = captureStateSnapshot();
    savedContentKey = snapshot ? contentKey(snapshot) : savedContentKey;
    fileNameEl.textContent = state.fileName;
    syncHistoryButtons();
    setStatus("Saved current package bytes.");
  } catch (error) {
    showError(error);
  }
}

function outputFileName(fileName: string, forceMcd: boolean): string {
  if (forceMcd || !fileName.toLowerCase().endsWith(".mcd")) {
    return `${fileName.replace(/\.[^.]+$/, "") || "document"}.mcd`;
  }
  return fileName;
}

function setStatus(message: string): void {
  statusLine.textContent = message;
}

function clearDiagnostics(): void {
  diagnosticsEl.innerHTML = "";
}

function showError(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  setStatus(message);
  const node = document.createElement("div");
  node.className = "diagnostic";
  node.textContent = message;
  diagnosticsEl.appendChild(node);
}

function revokeAssetUrls(): void {
  for (const url of assetUrls) {
    URL.revokeObjectURL(url);
  }
  assetUrls = [];
}

function nextAnnotationId(packageState: PackageState): string {
  const existing = new Set(packageState.annotations.map((annotation) => annotation.id));
  for (let index = 1; ; index += 1) {
    const id = `annotation-${String(index).padStart(4, "0")}`;
    if (!existing.has(id)) {
      return id;
    }
  }
}

function sanitizeId(value: string): string {
  const cleaned = value
    .trim()
    .replace(/^[^A-Za-z0-9]+/, "")
    .replace(/[^A-Za-z0-9_.-]/g, "-");
  return cleaned || "annotation";
}

function capitalize(value: string): string {
  return `${value[0]?.toUpperCase() ?? ""}${value.slice(1)}`;
}

function options(values: string[], selected: string): string {
  return values
    .map(
      (value) =>
        `<option value="${escapeAttr(value)}"${value === selected ? " selected" : ""}>${escapeHtml(
          value,
        )}</option>`,
    )
    .join("");
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function escapeAttr(value: string): string {
  return escapeHtml(value).replaceAll('"', "&quot;");
}
