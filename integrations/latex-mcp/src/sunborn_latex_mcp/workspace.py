from pathlib import Path, PurePosixPath
import os
import tempfile


class WorkspaceError(ValueError):
    pass


class Workspace:
    def __init__(self, root: Path, max_bytes: int = 4_000_000):
        self.root = root.expanduser().resolve(strict=True)
        if not self.root.is_dir():
            raise WorkspaceError("Workspace root must be a directory")
        self.max_bytes = max_bytes

    def resolve(self, name: str, must_exist: bool = False) -> Path:
        if not name or "\x00" in name or "\\" in name:
            raise WorkspaceError("Path must be a non-empty POSIX relative path")
        rel = PurePosixPath(name)
        if rel.is_absolute() or any(part in ("", ".", "..") for part in rel.parts):
            raise WorkspaceError("Absolute paths and traversal segments are forbidden")
        candidate = self.root
        for part in rel.parts:
            candidate = candidate / part
            if candidate.is_symlink():
                raise WorkspaceError("Symlinks are forbidden inside the manuscript workspace")
        resolved = candidate.resolve(strict=must_exist)
        if not resolved.is_relative_to(self.root):
            raise WorkspaceError("Path escapes the manuscript workspace")
        if must_exist and not resolved.is_file():
            raise WorkspaceError("Expected a regular file")
        return resolved

    def list_files(self, limit: int = 500) -> list[str]:
        if not 1 <= limit <= 2000:
            raise WorkspaceError("limit must be between 1 and 2000")
        result = []
        for path in sorted(self.root.rglob("*")):
            if path.is_symlink():
                raise WorkspaceError("Symlink found in manuscript workspace")
            rel = path.relative_to(self.root)
            if path.is_file() and not any(
                part in {".git", ".venv", "__pycache__"} for part in rel.parts
            ):
                result.append(rel.as_posix())
                if len(result) >= limit:
                    break
        return result

    def read(self, name: str) -> str:
        path = self.resolve(name, must_exist=True)
        if path.suffix.lower() not in {".tex", ".bib", ".sty", ".cls", ".md", ".txt", ".log", ".blg"}:
            raise WorkspaceError("Unsupported text file extension")
        if path.stat().st_size > self.max_bytes:
            raise WorkspaceError("File exceeds configured size limit")
        return path.read_text(encoding="utf-8", errors="replace")

    def write(self, name: str, content: str, create_only: bool = False) -> dict:
        if len(content.encode("utf-8")) > self.max_bytes:
            raise WorkspaceError("Content exceeds configured size limit")
        path = self.resolve(name)
        if path.suffix.lower() not in {".tex", ".bib", ".sty", ".cls", ".md", ".txt"}:
            raise WorkspaceError("Writes are restricted to TeX, BibTeX, style, class, Markdown and text files")
        path.parent.mkdir(parents=True, exist_ok=True)
        path = self.resolve(name)
        if create_only and path.exists():
            raise WorkspaceError("File already exists; refusing to overwrite")
        fd, temporary = tempfile.mkstemp(prefix=".sunborn-", dir=path.parent)
        try:
            with os.fdopen(fd, "w", encoding="utf-8", newline="") as stream:
                stream.write(content)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, path)
        except Exception:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass
            raise
        return {"path": name, "bytes": len(content.encode("utf-8"))}
