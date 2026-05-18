Best default for `.mcd`:

```text
Code:          Apache License 2.0
Spec/docs:     CC BY 4.0
Schemas/tests: CC0 or Apache 2.0
Name/logo:     trademark policy, not open-source license
```

## Recommended choice

Use **Apache License 2.0** for the implementation:

```text
mcd-core
mcd-cli
mcd-python
mcd-wasm
mcd-js
reference renderer
validators
```

Apache 2.0 is better than MIT for this project because `.mcd` is a **file format and parser ecosystem**, and future patent concerns are possible around parsing, validation, rendering, table anchoring, or page mapping. Apache 2.0 includes an explicit copyright license and an explicit patent license from contributors. ([apache.org][1])

A good `LICENSE` setup:

```text
LICENSE
  Apache License 2.0

NOTICE
  Markdown CSV Document / MCD
  Copyright ...
```

In `Cargo.toml`:

```toml
license = "Apache-2.0"
```

In `pyproject.toml`:

```toml
license = "Apache-2.0"
```

In `package.json`:

```json
{
  "license": "Apache-2.0"
}
```

## Why not MIT?

MIT is very permissive and widely used. It allows use, copying, modification, distribution, sublicensing, and sale, provided the copyright and license notice are included. ([Open Source Initiative][2])

But MIT does **not** contain the same explicit patent grant wording as Apache 2.0. For a serious open file format, Apache 2.0 is safer because companies, governments, and infrastructure projects usually care about patent clarity.

Use MIT only if your main goal is maximum simplicity:

```text
MIT = shortest and simplest
Apache 2.0 = still permissive, but better legal protection
```

For `.mcd`, I would choose **Apache 2.0**.

## Spec and documentation

For the `.mcd` specification, `ABOUT.md`, examples, diagrams, and explanatory documentation, use:

```text
Creative Commons Attribution 4.0 International
SPDX: CC-BY-4.0
```

CC BY 4.0 allows people to share and adapt the material, including commercially, as long as they provide attribution and indicate changes. ([Creative Commons][3])

That works well for:

```text
ABOUT.md
SPEC.md
TECH_STACK.md
implementation guides
diagrams
website content
tutorials
```

Add this to docs:

```markdown
Except where otherwise noted, this specification and documentation are licensed under
Creative Commons Attribution 4.0 International (CC BY 4.0).
```

## Schemas, examples, and test fixtures

For maximum adoption, use **CC0** for things people need to copy freely:

```text
JSON schemas
example .mcd files
sample CSV files
conformance test fixtures
golden parser outputs
```

CC0 is designed as a public-domain dedication where possible, reducing reuse friction. ([Creative Commons][4])

Recommended:

```text
schemas/       CC0-1.0
examples/      CC0-1.0
tests/fixtures CC0-1.0
```

This matters because implementers may want to copy schema files directly into their products. Attribution requirements on schemas can create unnecessary friction.

## Avoid GPL/AGPL for the reference implementation

GPL or AGPL would be “more open” in a copyleft sense, but they are probably wrong for `.mcd`.

A file format needs broad implementation:

```text
open-source tools
commercial tools
AI agents
document-management systems
government systems
browser integrations
office-suite plugins
cloud parsers
```

A strong copyleft license may discourage commercial or embedded adoption. For a format, adoption is usually more important than forcing every downstream implementation to be open source.

Use GPL/AGPL only if your priority is:

```text
All derivative software must remain open-source.
```

For `.mcd`, the better priority is:

```text
Anyone can implement, embed, validate, parse, render, and distribute it.
```

So choose Apache 2.0.

## Trademark

Do **not** try to handle the project name through the open-source license.

Keep:

```text
MCD
Markdown CSV Document
.mcd logo
certification marks
```

under a simple trademark policy.

Reason: you want people to freely implement the format, but you do **not** want incompatible implementations claiming to be official or conforming.

Recommended wording:

```text
The code is open-source under Apache-2.0.
The specification is open under CC-BY-4.0.
The MCD name and logo are governed by the MCD trademark policy.
Anyone may use “MCD-compatible” for implementations that pass the published conformance tests.
```

## Final licensing model

```text
Software implementation:
  Apache-2.0

Specification and documentation:
  CC-BY-4.0

Schemas, examples, fixtures:
  CC0-1.0

Project name, logo, certification wording:
  Trademark policy

Contributor process:
  DCO or CLA
```

## Short recommendation

Use this:

```text
Apache-2.0 for all code.
CC-BY-4.0 for the spec and documentation.
CC0-1.0 for schemas, examples, and test fixtures.
```

This gives `.mcd` the best balance of:

```text
full open-source availability
commercial adoption
patent clarity
low integration friction
standardization potential
AI-agent ecosystem compatibility
```

[1]: https://www.apache.org/licenses/LICENSE-2.0?utm_source=chatgpt.com "Apache License, Version 2.0"
[2]: https://opensource.org/license/mit?utm_source=chatgpt.com "The MIT License"
[3]: https://creativecommons.org/licenses/by/4.0/deed.en?utm_source=chatgpt.com "Deed - Attribution 4.0 International"
[4]: https://creativecommons.org/publicdomain/zero/1.0/deed.en?utm_source=chatgpt.com "Deed - CC0 1.0 Universal"
