import logging
import os
from pathlib import Path
from mcp.server import MCPServer
from .workspace import Workspace, WorkspaceError
from .compiler import compile_pdf, CompileError

logging.basicConfig(level=logging.INFO)
mcp = MCPServer("sunborn-latex")

def workspace() -> Workspace:
    root = os.environ.get("SUNBORN_LATEX_WORKSPACE")
    if not root:
        raise RuntimeError("SUNBORN_LATEX_WORKSPACE is required")
    return Workspace(Path(root), int(os.environ.get("SUNBORN_LATEX_MAX_SOURCE_BYTES", "2000000")))

@mcp.tool()
def latex_workspace_list(limit: int = 500) -> dict:
    """List files in the explicitly configured LaTeX workspace."""
    try: return {"files": workspace().list_files(limit)}
    except (WorkspaceError, RuntimeError) as e: return {"error": str(e)}

@mcp.tool()
def latex_read_file(path: str) -> dict:
    """Read an approved TeX, BibTeX, style, class, Markdown, or text file."""
    try: return {"path": path, "content": workspace().read(path)}
    except (WorkspaceError, OSError, RuntimeError) as e: return {"error": str(e)}

@mcp.tool()
def latex_create_file(path: str, content: str) -> dict:
    """Create a new TeX, BibTeX, Markdown, or text file without overwriting."""
    try: return workspace().write(path, content, True)
    except (WorkspaceError, OSError, RuntimeError) as e: return {"error": str(e)}

@mcp.tool()
def latex_update_file(path: str, content: str) -> dict:
    """Update an approved text file in the configured workspace."""
    try: return workspace().write(path, content, False)
    except (WorkspaceError, OSError, RuntimeError) as e: return {"error": str(e)}

@mcp.tool()
def latex_validate_project(main_file: str) -> dict:
    """Check basic LaTeX document structure; this is a heuristic, not a TeX parser."""
    try:
        text = workspace().read(main_file)
        required = ["\\documentclass", "\\begin{document}", "\\end{document}"]
        missing = [x for x in required if x not in text]
        return {"valid_structure": not missing, "missing_markers": missing,
                "note": "Static heuristic only; compile and inspect output for full validation."}
    except (WorkspaceError, OSError, RuntimeError) as e: return {"error": str(e)}

@mcp.tool()
def latex_compile_pdf(main_file: str) -> dict:
    """Compile with fixed latexmk policy; disabled unless explicitly enabled in a sandbox."""
    try: return compile_pdf(workspace(), main_file)
    except (CompileError, WorkspaceError, OSError, RuntimeError) as e:
        return {"status": "failed", "error": str(e)}

def main() -> None:
    mcp.run(transport="stdio")

if __name__ == "__main__":
    main()
