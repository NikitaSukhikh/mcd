# Format Improvement Implementation Plan

This plan turns the improvement checklist in `FOTMAT_IMPROVEMENTS_SUGGESTIONS.md` into a practical implementation sequence.

## 1. Table Primary Keys

Add `primaryKey` to table schemas and validate uniqueness across CSV rows.

Why: row-level identity is the foundation for stable annotations, citations, chart point references, row-level diffs, and reliable exports.

## 2. Table Foreign Keys

Add `foreignKeys` to table schemas and validate references across manifest-declared tables.

Why: table relationships should be explicit and machine-checkable instead of inferred from similar column names.

## 3. External Data References

Add manifest-side declarations for data that lives outside the package, including URI, media type, hash, size, and access notes.

Why: large datasets should be referenced deterministically without bloating `.mcd` packages.

## 4. Unified Provenance Metadata

Introduce a package-level provenance sidecar for source documents, extraction tools, generated assets, actors, hashes, and timestamps.

Why: auditability needs one consistent model instead of scattered source spans and hashes.

## 5. Unit Model

Replace free-form display-only units with a constrained semantic unit model for table columns and chart encodings.

Why: engineering and scientific documents need units that can be validated, converted, and compared.

Decision: keep canonical measurement units constrained, but allow free-form unit text only where it cannot silently change machine semantics.

- Table column schemas should use a structured semantic unit field for numeric measured values, for example a known unit code, dimension, scale, and optional system/reference.
- For the first implementation, keep this intentionally small: `unit.code` plus optional `unit.label`, or `unit.custom: true` plus required `unit.label`.
- Table views and chart encodings may keep free-form `unitLabel` display labels such as `"$"`, `"approx. kg"`, `"index points"`, or source-specific wording, but these labels must not be used for conversion or validation.
- Unknown or domain-specific units may be represented as a structured custom unit with a required free-form label and no automatic conversion unless an explicit conversion rule is supplied later.
- Source/provenance metadata may keep the original unit text exactly as extracted.
- Raw CSV cells should remain typed data, not mixed number-and-unit strings, unless the schema column type is `string`.

## 6. Version and Diff Model

Define stable revision metadata and patch/diff sidecars for controlled review workflows.

Why: proposed changes and review decisions should be reproducible across package versions.

## 7. Permission and Security Metadata

Add optional package metadata for classification, license, PII/PHI flags, retention, and allowed operations.

Why: enterprise, clinical, and regulated use cases need machine-readable handling constraints.
