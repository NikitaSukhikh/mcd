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

## 6. Version and Diff Model

Define stable revision metadata and patch/diff sidecars for controlled review workflows.

Why: proposed changes and review decisions should be reproducible across package versions.

## 7. Permission and Security Metadata

Add optional package metadata for classification, license, PII/PHI flags, retention, and allowed operations.

Why: enterprise, clinical, and regulated use cases need machine-readable handling constraints.
