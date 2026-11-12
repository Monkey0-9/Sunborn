import hashlib
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from .workspace import Workspace, WorkspaceError

class CompileError(RuntimeError):
    pass

def compile_pdf(workspace: Workspace, main_file: str) -> dict:
    if os.environ.get("SUNBORN_LATEX_ALLOW_COMPILE") != "1":
        raise CompileError("Compilation disabled; enable only inside an OS/container sandbox")
    source = workspace.resolve(main_file, True)
    if source.suffix.lower() != ".tex":
        raise WorkspaceError("Main file must end in .tex")
    latexmk = os.environ.get("SUNBORN_LATEXMK", "latexmk")
    if not shutil.which(latexmk):
        raise CompileError("latexmk executable not found")
    timeout = min(max(int(os.environ.get("SUNBORN_LATEX_TIMEOUT", "90")), 5), 180)
    max_bytes = min(max(int(os.environ.get("SUNBORN_LATEX_MAX_BYTES", "50000000")), 1024), 100000000)
    with tempfile.TemporaryDirectory(prefix="sunborn-latex-") as tmp:
        root = Path(tmp)
        src = root / "src"
        out = root / "out"
        src.mkdir()
        out.mkdir()
        total = 0
        for item in source.parent.rglob("*"):
            if item.is_symlink():
                raise CompileError("Symlinks forbidden in compile inputs")
            rel = item.relative_to(source.parent)
            if any(x in {".git", ".venv", "build", "dist"} for x in rel.parts):
                continue
            target = src / rel
            if item.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            elif item.is_file():
                total += item.stat().st_size
                if total > max_bytes:
                    raise CompileError("Compilation inputs exceed size limit")
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(item, target)
        copied = src / source.name
        cmd = [
            latexmk, "-norc", "-pdf", "-interaction=nonstopmode",
            "-halt-on-error", "-file-line-error", f"-outdir={out}",
            "-pdflatex=pdflatex -no-shell-escape %O %S", copied.name
        ]
        env = {
            "PATH": os.environ.get("PATH", ""),
            "HOME": str(root), "TMPDIR": str(root), "TEMP": str(root),
            "TEXMFOUTPUT": str(out), "openin_any": "p", "openout_any": "p"
        }
        try:
            run = subprocess.run(cmd, cwd=src, env=env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                 text=True, timeout=timeout, shell=False, check=False)
        except subprocess.TimeoutExpired as exc:
            raise CompileError("latexmk timed out") from exc
        log = (run.stdout or "")[-20000:]
        if run.returncode:
            raise CompileError(f"latexmk exit {run.returncode}:\n{log}")
        pdf = out / f"{source.stem}.pdf"
        if not pdf.is_file():
            raise CompileError("No expected PDF artifact produced")
        size = pdf.stat().st_size
        if size > max_bytes:
            raise CompileError("PDF exceeds artifact size limit")
        return {"status": "success", "pdf_name": pdf.name, "bytes": size,
                "sha256": hashlib.sha256(pdf.read_bytes()).hexdigest(),
                "log_tail": log, "artifact": "ephemeral"}
