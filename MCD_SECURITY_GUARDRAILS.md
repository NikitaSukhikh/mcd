# MCD Security Guardrails

This document describes the security guardrails currently implemented in the
MCD codebase.

## Implemented Scope

The current implementation provides security at these layers:

- ZIP/package opening
- package-internal path validation
- root mimetype validation
- manifest path and identifier validation
- external data declaration validation
- image asset validation
- SVG safety validation
- annotation target validation
- provenance metadata validation
- optional SHA-256 hashes for image assets, external data declarations, and provenance records

The implementation does not provide file-level access control, DRM, encryption,
package-level signatures, or package-wide checksum verification.

## 1. Safe Package Opening

Implemented in:

- `crates/mcd-core/src/package.rs`

The package reader treats `.mcd` input as untrusted archive input.

When opening a ZIP package, the reader currently enforces:

- maximum package entry count: `10_000`
- maximum single decompressed file size: `64 MiB`
- maximum total decompressed package size: `512 MiB`
- root `mimetype` entry is present
- root `mimetype` entry is valid UTF-8
- root `mimetype` value equals `application/vnd.mcd+zip`, ignoring trailing CR/LF

The reader skips directory entries and stores validated file entries in memory.

If ZIP parsing fails and the input is valid UTF-8 that does not start with `PK`,
the implementation treats the input as a plain Markdown `.mcd` candidate and
wraps it as a minimal package with:

- `mimetype`
- `manifest.json`
- `content/main.md`

## 2. Internal Path Validation

Implemented in:

- `crates/mcd-core/src/package.rs`
- reused by manifest, annotation, provenance, and asset validators

Package paths are validated by `validate_internal_path`.

The implementation rejects paths that:

- are empty
- start with `/`
- start with `\`
- contain `\`
- contain `:`
- contain a null byte
- contain an empty path component
- contain `.`
- contain `..`

The validator builds a normalized UTF-8 path and requires the normalized path to
match the original input. Any mismatch is rejected.

The ZIP reader also rejects duplicate paths after normalization using a
case-insensitive duplicate key. This prevents packages from containing ambiguous
entries such as:

```text
manifest.json
Manifest.json
```

Those entries may behave differently on different filesystems, so the package
reader rejects them.

These checks directly address path traversal and ambiguous archive-entry names.

## 3. Manifest-Level Validation

Implemented in:

- `crates/mcd-core/src/manifest.rs`

The manifest parser validates the basic structure before other package content
is loaded.

The current implementation enforces:

- `format` must be `MCD`
- `version` must be `0.1`
- `entrypoint` must be a safe internal package path
- table IDs cannot be empty
- table IDs must be unique
- table data paths must be safe internal package paths
- table schema paths must be safe internal package paths
- table view paths must be safe internal package paths
- image IDs cannot be empty
- image IDs must be unique
- image metadata paths must be safe internal package paths
- annotation IDs cannot be empty
- annotation IDs must be unique
- annotation metadata paths must be safe internal package paths
- declared asset paths must be safe internal package paths
- provenance sidecar path, if present, must be a safe internal package path
- layout styles path, if present, must be a safe internal package path
- layout page map path, if present, must be a safe internal package path

This means manifest-declared paths are not trusted as raw strings. They must pass
the same internal path rules used by the package reader.

## 4. External Data Declaration Validation

Implemented in:

- `crates/mcd-core/src/manifest.rs`

External data declarations are metadata only. The current validator validates
their shape but does not fetch external resources.

The current implementation enforces:

- external data IDs cannot be empty
- external data IDs must be unique
- external data IDs must start with an ASCII alphanumeric character
- external data IDs may contain ASCII alphanumerics, `_`, `.`, and `-`
- URI must be absolute and contain no whitespace
- URI scheme must be one of `http`, `https`, `s3`, `gs`, `file`, or `ipfs`
- URI must use a `//` authority-style form after the scheme
- media type must contain a non-empty type and subtype separated by `/`
- optional hash must use `sha256:<64 lowercase hex characters>`
- optional description cannot be empty or whitespace-only
- optional access notes cannot be empty or whitespace-only

The access object can declare:

- `requiresNetwork`
- `requiresAuthentication`
- `notes`

These fields are validated as metadata. They do not trigger network access.

## 5. Image Asset Validation

Implemented in:

- `crates/mcd-core/src/images.rs`
- `crates/mcd-core/src/assets.rs`

Image metadata is loaded only from manifest-declared image metadata files.

The current implementation enforces:

- image metadata file must exist
- image metadata JSON must parse
- image metadata `id` must match the image ID declared in the manifest
- image metadata `id` cannot be empty
- image intrinsic size metadata must be valid
- image role and text metadata must satisfy the image validation rules
- meaningful-content metadata must be valid
- referenced image asset path must be a safe internal package path
- image asset must be under `assets/` or under a manifest-declared asset path
- referenced image asset must exist in the package
- declared asset media type must match path-based media type detection
- declared media type must start with `image/`
- optional image asset hash must match the actual package bytes

Hash validation for image assets uses:

```text
sha256:<64 lowercase hex characters>
```

If the declared hash is malformed or does not match the asset bytes, validation
fails.

## 6. SVG Safety Validation

Implemented in:

- `crates/mcd-core/src/assets.rs`

SVG files receive additional validation when an image asset declares
`image/svg+xml`.

The current implementation enforces:

- SVG bytes must be valid UTF-8
- SVG content must parse as XML

The validator rejects these SVG elements:

- `script`
- `foreignObject`
- `animate`
- `animateMotion`
- `animateTransform`
- `set`

The validator rejects any SVG attribute whose name starts with `on`, such as:

- `onclick`
- `onload`
- `onmouseover`

The validator rejects attribute values containing:

- `http://`
- `https://`
- `//`
- `javascript:`
- `data:`

This means the current SVG policy rejects active SVG content and common external
reference forms for manifest-declared image assets.

## 7. Annotation Validation

Implemented in:

- `crates/mcd-core/src/annotations.rs`

Annotation metadata is loaded only from manifest-declared annotation metadata
files.

The current implementation enforces:

- annotation metadata file must exist
- annotation metadata JSON must parse
- annotation metadata `id` must match the annotation ID declared in the manifest
- annotation metadata `id` cannot be empty
- annotation body cannot be empty or whitespace-only
- annotation labels cannot be empty or whitespace-only
- annotation target must resolve
- Markdown annotation markers must reference declared annotation metadata

Supported annotation targets are validated as follows:

- `document`: always valid
- `block`: target block ID must exist in the parsed document
- `placement`: placement ref must exist in a table or image directive
- `table`: table ID must exist in the manifest
- `image`: image ID must exist in the manifest
- `path`: path must be safe and must exist in the package

For proposed textual changes, the implementation enforces:

- proposed-change path must be a safe internal package path
- proposed-change text cannot be empty or whitespace-only

Proposed changes are validated as metadata. They are not automatically applied
by package validation.

## 8. Provenance Validation

Implemented in:

- `crates/mcd-core/src/provenance.rs`

Provenance metadata is loaded only when `manifest.json` declares a provenance
sidecar path.

The current implementation enforces:

- provenance sidecar path must be safe according to manifest validation
- provenance sidecar file must exist
- provenance JSON must parse
- provenance IDs must use the supported ID format
- duplicate provenance IDs are rejected within each provenance collection
- provenance source must declare either `path` or `uri`
- provenance source `path`, if present, must be safe and must exist in the package
- provenance source `uri`, if present, must use a supported external URI form
- provenance media type, if present, must be valid
- provenance hash, if present, must use `sha256:<64 lowercase hex characters>`
- provenance title, if present, cannot be empty or whitespace-only
- provenance timestamps, if present, must parse as RFC 3339 timestamps
- actor URI, if present, must use a supported external URI form
- tool name cannot be empty
- tool URI and hash, if present, must be valid
- generated asset path must be safe and must exist in the package
- generated asset media type, hash, and timestamp fields must be valid
- generated asset source/tool/actor references must resolve
- activity timestamps must be valid RFC 3339 timestamps
- activity end time cannot be earlier than activity start time
- activity actor/tool/source/input/output references must resolve
- duplicate references in provenance reference lists are rejected

Provenance validation provides audit metadata checks. It does not enforce access
control or usage permissions.

## 9. Whole-Package Validation Flow

Implemented in:

- `crates/mcd-core/src/validate.rs`

`validate_package` currently performs this flow:

1. Parse and validate `manifest.json`.
2. Load the Markdown entrypoint as an MCD document.
3. Load manifest-declared tables.
4. Validate table foreign keys.
5. Load and validate table views.
6. Validate table anchors in the document.
7. Load and validate manifest-declared images.
8. Validate image anchors in the document.
9. Load and validate manifest-declared annotations.
10. Validate Markdown annotation markers.
11. Load and validate manifest-declared provenance metadata.

Validation returns structured diagnostics when these checks fail.

## 10. Implemented Hash Checks

The current implementation validates hashes in these places:

- image asset hash in image metadata
- external data declaration hash format
- provenance source/tool/generated asset hash format

Only image asset hashes are checked against actual package bytes.

External data hashes and provenance hashes are currently validated for correct
`sha256:<64 lowercase hex characters>` syntax. They are not fetched or compared
against remote bytes during normal validation.

## 11. Not Implemented

The following items are not implemented as security guarantees in the current
codebase:

- package encryption
- password protection
- user or group access control
- DRM
- prevention of copying, screenshots, or downstream extraction
- policy enforcement for PII, PHI, retention, or allowed operations
- package-wide `integrity/checksums.json` validation
- package-wide `integrity/signatures.json` validation
- digital signature verification
- immutable render/source hash chain validation
- automatic remote-resource fetching for validation
- sandboxed executable or interactive content
- JavaScript execution inside `.mcd` packages
- macro execution

The manifest enum includes an `MCD-Signed` profile value, and the design docs
mention checksums and signatures, but package-level signature verification is
not currently implemented.

## 12. Security Boundary Summary

The current security boundary is:

```text
Open package safely.
Reject unsafe internal paths.
Validate declared package structure.
Validate assets before rendering.
Reject active or externally-referencing SVG image assets.
Validate metadata references and hash syntax.
Validate image hashes against local bytes when declared.
```

The current security boundary is not:

```text
Restrict who can open the file.
Restrict who can copy the file.
Enforce enterprise data-handling policy by itself.
Prove package authorship with signatures.
Encrypt sensitive content.
Run trusted interactive code.
```

"Permission and Security Metadata" is not implemented as an enforcement feature
in the current codebase. Any policy enforcement for classification, retention,
PII/PHI handling, allowed operations, or access control currently has to happen
outside the `.mcd` package format.
