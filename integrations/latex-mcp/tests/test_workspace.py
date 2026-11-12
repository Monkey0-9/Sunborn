from pathlib import Path
import pytest
from sunborn_latex_mcp.workspace import Workspace, WorkspaceError

@pytest.fixture
def ws(tmp_path: Path) -> Workspace:
    return Workspace(tmp_path)

@pytest.mark.parametrize("bad", ["../secret.tex", "/etc/passwd", "a/../../b.tex", ""])
def test_path_traversal_rejected(ws: Workspace, bad: str) -> None:
    with pytest.raises(WorkspaceError):
        ws.resolve(bad)

def test_create_only_is_non_overwriting(ws: Workspace) -> None:
    ws.write("paper.tex", "first", True)
    with pytest.raises(WorkspaceError):
        ws.write("paper.tex", "second", True)
    assert ws.read("paper.tex") == "first"

def test_unsupported_write_extension_rejected(ws: Workspace) -> None:
    with pytest.raises(WorkspaceError):
        ws.write("payload.py", "print(1)")
