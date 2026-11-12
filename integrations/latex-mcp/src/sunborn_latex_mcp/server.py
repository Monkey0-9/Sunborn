import logging
import os
from pathlib import Path

from mcp.server.fastmcp import FastMCP

from .workspace import Workspace, WorkspaceError
from .compiler import compile_pdf, CompileError

logging.basicConfig(level=logging.INFO)
mcp = FastMCP("sunborn-paper-latex")


def workspace() -> Workspace:
    root = os.environ.get("SUNBORN_LATEX_WORKSPACE")
    if not root:
        raise RuntimeError("SUNBORN_LATEX_WORKSPACE must point to the paper directory")
    return Workspace(
        Path(root),
        int(os.environ.get("SUNBORN_LATEX_MAX_SOURCE_BYTES", "4000000")),
    )


@mcp.tool()
def latex_workspace_list(limit: int = 500) -> dict:
    """List LaTeX manuscript files in the configured paper directory."""
    try:
        return {"files": workspace().list_files(limit)}
    except (WorkspaceError, RuntimeError) as exc:
        return {"error": str(exc)}


@mcp.tool()
def latex_read_file(path: str) -> dict:
    """Read a .tex, .bib, .sty, .cls, .md, .txt, or supported figure metadata file."""
    try:
        return {"path": path, "content": workspace().read(path)}
    except (WorkspaceError, OSError, RuntimeError) as exc:
        return {"error": str(exc)}


@mcp.tool()
def latex_create_file(path: str, content: str) -> dict:
    """Create a new manuscript source file without overwriting an existing file."""
    try:
        return workspace().write(path, content, create_only=True)
    except (WorkspaceError, OSError, RuntimeError) as exc:
        return {"error": str(exc)}


@mcp.tool()
def latex_update_file(path: str, content: str) -> dict:
    """Write or replace a manuscript source file in the configured paper directory."""
    try:
        return workspace().write(path, content, create_only=False)
    except (WorkspaceError, OSError, RuntimeError) as exc:
        return {"error": str(exc)}


@mcp.tool()
def latex_validate_project(main_file: str = "main.tex") -> dict:
    """Check basic LaTeX structure and referenced local bibliography files."""
    try:
        ws = workspace()
        source = ws.read(main_file)
        required = ["\\documentclass", "\\begin{document}", "\\end{document}"]
        missing = [marker for marker in required if marker not in source]
        bib_names = []
        for marker in ("\\bibliography{", "\\addbibresource{"):
            if marker in source:
                tail = source.split(marker, 1)[1].split("}", 1)[0].strip()
                bib_names.extend([name.strip() for name in tail.split(",") if name.strip()])
        missing_bib = []
        for name in bib_names:
            candidates = [name] if name.endswith(".bib") else [name + ".bib"]
            if not any(ws.resolve(candidate).is_file() for candidate in candidates):
                missing_bib.append(name)
        return {
            "valid_structure": not missing and not missing_bib,
            "missing_markers": missing,
            "bibliography_references": bib_names,
            "missing_bibliography_files": missing_bib,
            "note": "Static checks only; successful latexmk build and human review are still required.",
        }
    except (WorkspaceError, OSError, RuntimeError) as exc:
        return {"error": str(exc)}


@mcp.tool()
def latex_compile_pdf(main_file: str = "main.tex") -> dict:
    """Build the manuscript with latexmk and persist PDF and auxiliary files under build/."""
    try:
        return compile_pdf(workspace(), main_file)
    except (CompileError, WorkspaceError, OSError, RuntimeError) as exc:
        return {"status": "failed", "error": str(exc)}


def main() -> None:
    # FastMCP stdio transport: stdout is reserved for MCP protocol messages.
    mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
