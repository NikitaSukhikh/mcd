import { openMcd, type Diagnostic, type ValidationResult } from "@mcd/parser";
import DOMPurify from "dompurify";
import JSZip from "jszip";
import katex from "katex";
import { marked, type Tokens } from "marked";
import Papa from "papaparse";

import "katex/dist/katex.min.css";
import "./styles.css";

const MCD_MIMETYPE = "application/vnd.mcd+zip";
const textDecoder = new TextDecoder();
type ActiveTab = "text" | "tables" | "annotations";

marked.use({
  gfm: true,
  breaks: false,
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
  images?: unknown[];
  annotations?: AnnotationManifestEntry[];
  assets?: unknown[];
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

interface EditableTable {
  manifest: TableManifestEntry;
  schema: TableSchema;
  rows: Record<string, string>[];
}

interface EditableAnnotation {
  id: string;
  metadata: string;
  targetText: string;
  kind: string;
  status: string;
  body: string;
  author: string;
  labels: string;
  created: string;
  originalMetadata?: string;
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

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) {
  throw new Error("Missing #app root.");
}

app.innerHTML = `
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <span class="brand-mark">M</span>
        <div>
          <div class="brand-title">MCD Viewer</div>
          <div class="file-name" id="fileName">No document loaded</div>
        </div>
      </div>
      <div class="toolbar">
        <button id="openButton" type="button">Open</button>
        <button id="validateButton" type="button" disabled>Validate</button>
        <button id="saveButton" class="primary" type="button" disabled>Save .mcd</button>
      </div>
    </header>
    <main class="workspace">
      <section class="editor-pane">
        <input id="fileInput" class="hidden-input" type="file" accept=".mcd,application/zip,application/vnd.mcd+zip,text/markdown,text/plain" />
        <div id="dropZone" class="drop-zone">
          <div class="drop-title">Drop a .mcd file here</div>
          <div class="drop-copy">The file is parsed locally in this browser session.</div>
        </div>
        <div class="status-panel">
          <div class="status-line" id="statusLine">Open a document to edit Markdown, annotations, and CSV-backed tables.</div>
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
            <button id="addAnnotationButton" type="button" disabled>Add annotation</button>
          </div>
          <div class="list-stack" id="annotationsEditor"></div>
        </section>
      </section>
      <section class="preview-pane">
        <article class="preview-document" id="preview">
          <div class="empty-state">Preview appears after opening a document.</div>
        </article>
      </section>
    </main>
  </div>
`;

const fileNameEl = byId<HTMLDivElement>("fileName");
const fileInput = byId<HTMLInputElement>("fileInput");
const openButton = byId<HTMLButtonElement>("openButton");
const validateButton = byId<HTMLButtonElement>("validateButton");
const saveButton = byId<HTMLButtonElement>("saveButton");
const dropZone = byId<HTMLDivElement>("dropZone");
const statusLine = byId<HTMLDivElement>("statusLine");
const diagnosticsEl = byId<HTMLDivElement>("diagnostics");
const markdownEditor = byId<HTMLTextAreaElement>("markdownEditor");
const tablesEditor = byId<HTMLDivElement>("tablesEditor");
const annotationsEditor = byId<HTMLDivElement>("annotationsEditor");
const addAnnotationButton = byId<HTMLButtonElement>("addAnnotationButton");
const preview = byId<HTMLElement>("preview");

openButton.addEventListener("click", () => fileInput.click());
fileInput.addEventListener("change", () => {
  const file = fileInput.files?.[0];
  if (file) {
    void loadFile(file);
  }
  fileInput.value = "";
});

validateButton.addEventListener("click", () => {
  void renderAndValidate();
});

saveButton.addEventListener("click", () => {
  void saveDocument();
});

dropZone.addEventListener("dragover", (event) => {
  event.preventDefault();
  dropZone.classList.add("is-active");
});

dropZone.addEventListener("dragleave", () => {
  dropZone.classList.remove("is-active");
});

dropZone.addEventListener("drop", (event) => {
  event.preventDefault();
  dropZone.classList.remove("is-active");
  const file = event.dataTransfer?.files[0];
  if (file) {
    void loadFile(file);
  }
});

markdownEditor.addEventListener("input", () => {
  if (!state) {
    return;
  }
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
  const id = nextAnnotationId(state);
  state.annotations.push({
    id,
    metadata: `annotations/${id}.annotation.json`,
    targetText: JSON.stringify({ type: "document" }, null, 2),
    kind: "comment",
    status: "open",
    body: "New annotation",
    author: "",
    labels: "",
    created: new Date().toISOString(),
  });
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

async function loadFile(file: File): Promise<void> {
  setStatus(`Opening ${file.name}...`);
  clearDiagnostics();
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    state = await loadPackage(file.name, bytes);
    hydrateUiFromState();
    await renderAndValidate();
  } catch (error) {
    state = undefined;
    hydrateUiFromState();
    showError(error);
  }
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
          entrypoint: "content/main.md",
        },
        null,
        2,
      ),
    );
    zip.file("content/main.md", textDecoder.decode(bytes));
  }

  const manifest = await readManifest(zip);
  const markdown = await readText(zip, manifest.entrypoint);
  const tables = await readTables(zip, manifest.tables ?? []);
  const annotations = await readAnnotations(zip, manifest.annotations ?? []);
  const { pageMap, pageMapPath } = await readPageMap(zip, manifest);

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
      rows: parseCsvRows(csv, schema.columns),
    });
  }
  return tables;
}

async function readAnnotations(
  zip: JSZip,
  entries: AnnotationManifestEntry[],
): Promise<EditableAnnotation[]> {
  const annotations: EditableAnnotation[] = [];
  for (const entry of entries) {
    const raw = JSON.parse(await readText(zip, entry.metadata)) as Record<string, unknown>;
    annotations.push({
      id: String(raw.id ?? entry.id),
      metadata: entry.metadata,
      targetText: JSON.stringify(raw.target ?? { type: "document" }, null, 2),
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
    : "No document loaded";
  markdownEditor.disabled = !hasState;
  validateButton.disabled = !hasState;
  saveButton.disabled = !hasState;
  addAnnotationButton.disabled = !hasState;
  markdownEditor.value = state?.markdown ?? "";
  renderTablesEditor();
  renderAnnotationsEditor();
  if (!state) {
    setStatus("Open a document to edit Markdown, annotations, and CSV-backed tables.");
    preview.innerHTML = `<div class="empty-state">Preview appears after opening a document.</div>`;
  }
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
        const row = Object.fromEntries(table.schema.columns.map((column) => [column.name, ""]));
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

  state.annotations.forEach((annotation, index) => {
    const card = document.createElement("section");
    card.className = "item-card";
    card.innerHTML = `
      <div class="item-header">
        <div class="item-title">${escapeHtml(annotation.id)}</div>
        <button class="danger" type="button" data-field="remove">Remove</button>
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
    `;

    bindAnnotationInput(card, annotation, "id", (value) => {
      const previous = annotation.metadata;
      annotation.id = sanitizeId(value);
      annotation.metadata = `annotations/${annotation.id}.annotation.json`;
      if (previous !== annotation.metadata) {
        state?.removedAnnotationPaths.add(previous);
      }
    });
    bindAnnotationInput(card, annotation, "kind", (value) => {
      annotation.kind = value;
    });
    bindAnnotationInput(card, annotation, "status", (value) => {
      annotation.status = value;
    });
    bindAnnotationInput(card, annotation, "author", (value) => {
      annotation.author = value;
    });
    bindAnnotationInput(card, annotation, "body", (value) => {
      annotation.body = value;
    });
    bindAnnotationInput(card, annotation, "targetText", (value) => {
      annotation.targetText = value;
    });
    bindAnnotationInput(card, annotation, "labels", (value) => {
      annotation.labels = value;
    });
    bindAnnotationInput(card, annotation, "created", (value) => {
      annotation.created = value;
    });
    card
      .querySelector<HTMLButtonElement>('[data-field="remove"]')
      ?.addEventListener("click", () => {
        state?.removedAnnotationPaths.add(annotation.metadata);
        if (annotation.originalMetadata) {
          state?.removedAnnotationPaths.add(annotation.originalMetadata);
        }
        state?.annotations.splice(index, 1);
        renderAnnotationsEditor();
        markDirty();
      });
    annotationsEditor.appendChild(card);
  });
}

function bindAnnotationInput(
  root: HTMLElement,
  annotation: EditableAnnotation,
  field: keyof EditableAnnotation,
  update: (value: string) => void,
): void {
  const input = root.querySelector<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
    `[data-field="${field}"]`,
  );
  input?.addEventListener("input", () => {
    update(input.value);
    if (field === "id") {
      const title = root.querySelector<HTMLDivElement>(".item-title");
      if (title) {
        title.textContent = annotation.id;
      }
    }
    markDirty();
  });
}

function markDirty(): void {
  if (!state) {
    return;
  }
  state.dirty = true;
  fileNameEl.textContent = `${state.fileName} (edited)`;
  queueRender();
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
  clearDiagnostics();
  revokeAssetUrls();
  try {
    const bytes = await packageBytes();
    const doc = await openMcd(bytes);
    const validation = doc.validate();
    renderDiagnostics(validation);
    const markdown = validation.valid ? doc.markdown({ expandTables: true }) : state.markdown;
    await renderMarkdownPreview(markdown);
    setStatus(
      validation.valid
        ? "Document is valid. Preview is rendered from the current in-memory package."
        : "Document has validation errors. Preview is rendered from the Markdown editor.",
    );
  } catch (error) {
    await renderMarkdownPreview(state.markdown);
    showError(error);
  }
}

async function renderMarkdownPreview(markdown: string): Promise<void> {
  const rendered = marked.parse(markdown, { async: false }) as string;
  const sanitized = DOMPurify.sanitize(rendered, {
    USE_PROFILES: { html: true, mathMl: true },
    ADD_ATTR: ["target"],
  });
  renderPagedPreview(sanitized);
  enhancePreviewDom();
  await rewritePackageImageSources();
  await waitForPreviewImages();
  repaginatePreview();
}

function renderPagedPreview(html: string): void {
  const template = document.createElement("template");
  template.innerHTML = html;
  const nodes = Array.from(template.content.childNodes).filter((node) => {
    return node.nodeType !== Node.TEXT_NODE || Boolean(node.textContent?.trim());
  });

  preview.innerHTML = "";
  preview.classList.add("is-paged");

  const pageNumber = paginateNodes(nodes);
  if (nodes.length === 0) {
    const pageBody = preview.querySelector<HTMLDivElement>(".preview-page-body");
    if (pageBody) {
      pageBody.innerHTML = `<div class="empty-state">Document has no previewable content.</div>`;
    }
  }

  updatePageMapMetadata(pageNumber);
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
  updatePageMapMetadata(pageNumber);
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
  cursor.page.body.appendChild(node);
  if (!isPreviewPageOverflowing(cursor.page.body)) {
    return cursor;
  }

  const remainder =
    node instanceof HTMLElement ? splitParagraphElementToFit(cursor.page.body, node) : undefined;
  if (remainder) {
    return appendNodeToPreviewPage(remainder, nextPreviewPage(cursor));
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

function updatePageMapMetadata(pageCount: number): void {
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
        label: previous?.label ?? `Page ${number}`,
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
  packageState.removedAnnotationPaths.clear();

  for (const table of packageState.tables) {
    packageState.zip.file(table.manifest.data, tableToCsv(table));
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
  const output: Record<string, unknown> = {
    id: annotation.id,
    target: JSON.parse(annotation.targetText) as unknown,
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
    fileNameEl.textContent = state.fileName;
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
