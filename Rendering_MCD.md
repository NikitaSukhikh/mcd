# Rendering technology

1. Immediate: HTML + CSS paged rendering.

CSS Paged Media already defines page size, margins, page boxes, headers, footers, page numbering, and paginated layout concepts.


For .mcd, the renderer should create:

render/index.html
render/styles.css
render/assets/*

Then the browser displays it.

CLI + browser is best first, because it gives immediate usability:

mcd view report.mcd
mcd render report.mcd --html out/
mcd render report.mcd --pdf report.pdf

Advantages:

No browser extension permissions.
No OS-specific installer at first.
No native GUI complexity.
Works on Windows, macOS, and Linux.
Easy for developers and AI agents.
Easy to integrate into CI/CD.


2. Web viewer should come second

TO DO: Build a web viewer like:

https://viewer.mcd.org

User flow:

Open website.
Drag report.mcd into the page.
Viewer parses it locally.
Document renders in browser.

This is excellent for demos and adoption.

Architecture:

Rust mcd-core
→ WebAssembly
→ browser viewer
→ HTML/CSS paged rendering

Modern web apps can read local files after the user grants access through APIs such as the File System Access API. Google’s documentation describes it as allowing web apps to read or save changes directly to local files after user permission.

Important: the web viewer should be local-first. The .mcd file should not be uploaded to a server unless the user explicitly chooses that.