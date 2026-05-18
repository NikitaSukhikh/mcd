import { readdir, readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

import { McdParserError, openMcd } from "./index.js";

const fixtureRoot = new URL("../../../tests/fixtures/conformance/", import.meta.url);

async function fixtureBytes(name: string): Promise<Uint8Array> {
  return readFile(new URL(name, fixtureRoot));
}

describe("@mcd/parser", () => {
  it("opens MCD packages from ArrayBuffer bytes", async () => {
    const bytes = await fixtureBytes("valid-minimal.mcd");
    const arrayBuffer = bytes.buffer.slice(
      bytes.byteOffset,
      bytes.byteOffset + bytes.byteLength,
    );
    const doc = await openMcd(arrayBuffer);

    expect(doc.validate()).toEqual({ valid: true, diagnostics: [] });
    expect(doc.blocks()[0]).toMatchObject({
      type: "heading",
      text: "Minimal",
    });
    expect(doc.annotations()).toEqual([]);
    expect(doc.markdown({ expandTables: true })).toContain("# Minimal");
  });

  it("validates conformance fixtures from bytes", async () => {
    const fixtures = await readdir(fixtureRoot);
    const mcdFixtures = fixtures.filter((fixture) => fixture.endsWith(".mcd"));

    for (const fixture of mcdFixtures) {
      const doc = await openMcd(await fixtureBytes(fixture));
      const validation = doc.validate();

      expect(validation.valid, fixture).toBe(fixture.startsWith("valid-"));
      if (!fixture.startsWith("valid-")) {
        expect(validation.diagnostics.length, fixture).toBeGreaterThan(0);
        expect(validation.diagnostics[0]?.level, fixture).toBe("error");
      }
    }
  });

  it("throws structured diagnostics for document exports that cannot parse", async () => {
    const doc = await openMcd(await fixtureBytes("invalid-bad-mimetype.mcd"));

    expect(() => doc.blocks()).toThrow(McdParserError);
    try {
      doc.blocks();
    } catch (error) {
      expect((error as McdParserError).diagnostic.code).toBe(
        "package.mimetype.invalid",
      );
    }
  });
});
