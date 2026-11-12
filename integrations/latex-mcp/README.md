# Sunborn Research Paper — LaTeX MCP

This MCP server exists to let Gemini CLI (agy) agents directly create, read, edit, validate, and compile the Sunborn research paper's LaTeX source. It is a writing-workflow tool, not a new Sunborn runtime/platform subsystem.

## Repository layout

The manuscript lives at `research/paper/`. Point `SUNBORN_LATEX_WORKSPACE` at that directory only. Do not expose the entire repository to the MCP server.

## Requirements

- Python 3.11+
- uv
- Gemini CLI
- TeX Live with `latexmk` and `pdflatex`

## Install (Windows PowerShell)

From the repository root:

    uv sync --project integrations/latex-mcp
    New-Item -ItemType Directory -Force research\paper
    $env:SUNBORN_LATEX_WORKSPACE = (Resolve-Path "research\paper").Path
    $env:SUNBORN_LATEX_ALLOW_COMPILE = "1"

For a production or shared environment, run the MCP process and TeX build in a rootless container/VM with no network, CPU/memory/PID/disk limits, and only the paper directory mounted. TeX sources are executable input; no-shell-escape is defense-in-depth, not a complete sandbox. The above local setting is suitable only for trusted manuscript source under your control.

## Gemini CLI configuration

Merge the following server entry into the existing repository-root `.gemini/settings.json`. Preserve all existing settings and MCP servers:

    {
      "mcpServers": {
        "sunborn-paper": {
          "command": "uv",
          "args": [
            "run",
            "--project",
            "integrations/latex-mcp",
            "sunborn-latex-mcp"
          ],
          "timeout": 30000,
          "trust": false,
          "includeTools": [
            "latex_workspace_list",
            "latex_read_file",
            "latex_create_file",
            "latex_update_file",
            "latex_validate_project",
            "latex_compile_pdf"
          ]
        }
      }
    }

Set `SUNBORN_LATEX_WORKSPACE` in the environment inherited by Gemini CLI before launching it. Restart Gemini CLI and run `/mcp list`. Approve tools explicitly; do not use YOLO mode.

## Tools available to agents

- `latex_workspace_list`: list manuscript files.
- `latex_read_file`: read TeX/BibTeX/style/class/text files.
- `latex_create_file`: create new source without overwriting.
- `latex_update_file`: directly edit source files.
- `latex_validate_project`: static document and bibliography checks.
- `latex_compile_pdf`: run latexmk and persist PDF/auxiliary output in `research/paper/build/`.

The agents edit the actual files in `research/paper/`; there is no upload/download handoff or copy-paste step.

## Verify

    uv run --project integrations/latex-mcp ruff check integrations/latex-mcp/src integrations/latex-mcp/tests
    uv run --project integrations/latex-mcp pytest integrations/latex-mcp/tests -q

Check the compiler:

    latexmk --version
    pdflatex --version

Launch Gemini CLI from the repository root and run `/mcp list`. Ask an agent to call `latex_workspace_list`, create a small test .tex file, call `latex_validate_project`, then call `latex_compile_pdf`. Confirm the PDF exists under `research/paper/build/`.

## Paper integrity rules

- Claims must be grounded in source code, reproducible tests, benchmark artifacts, or cited literature.
- Agents must never invent benchmark results, security guarantees, comparisons, or citations.
- Every quantitative claim must link to a reproducible evidence artifact.
- Preserve raw benchmark outputs and record commit SHA, environment, command, and timestamp.
- Use separate agent review for technical correctness, methodology, citations, and LaTeX quality.
- Keep generated build files under `build/`; do not commit temporary auxiliary files unless the paper's release process requires them.
