# Sunborn LaTeX MCP

A separately scoped MCP server for research manuscripts. It does not connect to the Sunborn inference gateway and receives no gateway credentials.

## Requirements

Python 3.11+, uv, Gemini CLI. TeX Live with latexmk and pdflatex is required only for compilation.

## Install

From the Sunborn repository root:

    cd integrations/latex-mcp
    uv sync --extra dev

Create a dedicated manuscript directory. Do not expose the whole repository as the workspace.

Linux/macOS:

    export SUNBORN_LATEX_WORKSPACE="$PWD/../../research/manuscripts"
    export SUNBORN_LATEX_ALLOW_COMPILE=0

Windows PowerShell:

    $env:SUNBORN_LATEX_WORKSPACE = (Resolve-Path "..\..\research\manuscripts").Path
    $env:SUNBORN_LATEX_ALLOW_COMPILE = "0"

## Gemini CLI setup

Merge .gemini/settings.example.json into the repository-root .gemini/settings.json without deleting existing MCP entries. Restart Gemini CLI and run /mcp list. Keep trust false and use explicit tool approvals.

## Security model

- Workspace root is explicit and mandatory.
- Absolute paths, traversal segments, Windows separators, and symlinks are rejected.
- Writes are limited to TeX, BibTeX, Markdown, and text.
- Create is non-overwriting; writes use atomic replacement.
- Compiler arguments are fixed; shell=False; no caller-supplied command or flags.
- latexmk rc files are disabled and pdfLaTeX receives -no-shell-escape.
- Compilation is disabled by default.
- Build artifacts are ephemeral and the server returns only metadata and SHA-256.

TeX is executable input. Disabling shell escape is not a complete sandbox. Before setting SUNBORN_LATEX_ALLOW_COMPILE=1, run the MCP process inside a rootless container or VM with no network, read-only base filesystem, bounded CPU, memory, process count, and disk, and only the manuscript workspace mounted. Never enable compilation on a shared host without those controls.

## Verify

    uv run ruff check src tests
    uv run pytest -q
    uv run sunborn-latex-mcp

Use the official MCP Inspector to verify tool discovery and calls. MCP stdio uses stdout for protocol frames; application logs belong on stderr.

## Tools

- latex_workspace_list
- latex_read_file
- latex_create_file
- latex_update_file
- latex_validate_project
- latex_compile_pdf

## Promotion gates

Before calling this production-approved: add a rootless sandbox profile and adversarial TeX tests; pin a TeX Live image and test real builds; persist artifacts in a controlled artifact store; integrate audit events with Sunborn's authoritative audit contract; add agent identity/document ACLs; add bibliography and PDF content validation; add SBOM, lockfile, image signing, and CI security gates. This initial implementation is a restricted staging integration, not a multi-tenant production isolation claim.
