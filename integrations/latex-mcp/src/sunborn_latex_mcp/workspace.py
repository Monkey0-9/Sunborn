from pathlib import Path, PurePosixPath
import os
import tempfile

class WorkspaceError(ValueError):
    pass

class Workspace:
    def __init__(self, root: Path, max_bytes: int = 2_000_000):
        self.root = root.expanduser().resolve(strict=True)
        if not self.root.is_dir():
            raise WorkspaceError("Workspace root must be a directory")
        self.max_bytes = max_bytes

    def resolve(self, name: str, must_exist: bool = False) -> Path:
        if not name or "\x00" in name or "\\" in name:
            raise WorkspaceError("Invalid relative POSIX path")
        rel = PurePosixPath(name)
        if rel.is_absolute() or any(p in ("", ".", "..") for p in rel.parts):
            raise WorkspaceError("Absolute paths and traversal are forbidden")
        path = self.root
        for part in rel.parts:
            path = path / part
            if path.is_symlink():
                raise WorkspaceError("Symlinks are forbidden")
        resolved = path.resolve(strict=must_exist)
        if not resolved.is_relative_to(self.root):
            raise WorkspaceError("Path escapes workspace")
        if must_exist and not resolved.is_file():
            raise WorkspaceError("Expected a regular file")
        return resolved

    def list_files(self, limit: int = 500) -> list[str]:
        if not 1 <= limit <= 2000:
            raise WorkspaceError("limit must be 1..2000")
        found = []
        for p in sorted(self.root.rglob("*")):
            if p.is_symlink():
                raise WorkspaceError("Symlink found in workspace")
            if p.is_file() and not any(x in {".git", ".venv", "build"} for x in p.relative_to(self.root).parts):
                found.append(p.relative_to(self.root).as_posix())
                if len(found) >= limit:
                    break
        return found

    def read(self, name: str) -> str:
        p = self.resolve(name, True)
        if p.suffix.lower() not in {".tex", ".bib", ".sty", ".cls", ".md", ".txt"}:
            raise WorkspaceError("Unsupported text file extension")
        if p.stat().st_size > self.max_bytes:
            raise WorkspaceError("File too large")
        return p.read_text(encoding="utf-8")

    def write(self, name: str, content: str, create_only: bool = False) -> dict:
        if len(content.encode("utf-8")) > self.max_bytes:
            raise WorkspaceError("Content too large")
        p = self.resolve(name)
        if p.suffix.lower() not in {".tex", ".bib", ".md", ".txt"}:
            raise WorkspaceError("Writes restricted to tex, bib, md, txt")
        p.parent.mkdir(parents=True, exist_ok=True)
        p = self.resolve(name)
        if create_only and p.exists():
            raise WorkspaceError("Refusing to overwrite existing file")
        fd, tmp = tempfile.mkstemp(prefix=".sunborn-", dir=p.parent)
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(content)
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp, p)
        except Exception:
            try: os.unlink(tmp)
            except FileNotFoundError: pass
            raise
        return {"path": name, "bytes": len(content.encode("utf-8"))}
