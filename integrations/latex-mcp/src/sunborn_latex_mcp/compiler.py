import hashlib
import os
import shutil
import subprocess
from pathlib import Path

from .workspace import Workspace, WorkspaceError


class CompileError(RuntimeError):
    pass


def compile_pdf(workspace: Workspace, main_file: str = "main.tex") -> dict:
    if os.environ.get("SUNBORN_LATEX_ALLOW_COMPILE", "0") != "1":
        raise CompileError(
            "Compilation is disabled. Enable only for trusted manuscript sources "
            "and preferably inside an OS/container sandbox."
        )

    source = workspace.resolve(main_file, must_exist=True)
    if source.suffix.lower() != ".tex":
        raise WorkspaceError("Main document must be a .tex file")

    latexmk = os.environ.get("SUNBORN_LATEXMK", "latexmk")
    if not shutil.which(latexmk):
        raise CompileError(f"Compiler not found on PATH: {latexmk}")

    timeout = min(max(int(os.environ.get("SUNBORN_LATEX_TIMEOUT", "120")), 5), 600)
    max_bytes = min(max(int(os.environ.get("SUNBORN_LATEX_MAX_BUILD_BYTES", "100000000")), 1024), 500000000)
    build_dir = workspace.root / "build"
    build_dir.mkdir(exist_ok=True)
    build_dir = workspace.resolve("build")

    command = [
        latexmk,
        "-norc",
        "-pdf",
        "-interaction=nonstopmode",
        "-halt-on-error",
        "-file-line-error",
        f"-outdir={build_dir}",
        f"-auxdir={build_dir}",
        "-pdflatex=pdflatex -no-shell-escape %O %S",
        source.relative_to(workspace.root).as_posix(),
    ]
    env = {
        "PATH": os.environ.get("PATH", ""),
        "HOME": str(build_dir),
        "TMPDIR": str(build_dir),
        "TEMP": str(build_dir),
        "TEXMFOUTPUT": str(build_dir),
        "openin_any": "p",
        "openout_any": "p",
    }

    try:
        proc = subprocess.run(
            command,
            cwd=workspace.root,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=timeout,
            shell=False,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise CompileError(f"latexmk exceeded {timeout} seconds") from exc

    log = (proc.stdout or "")[-30000:]
    if proc.returncode != 0:
        raise CompileError(f"latexmk exited with {proc.returncode}:\n{log}")

    pdf = build_dir / f"{source.stem}.pdf"
    if not pdf.is_file():
        raise CompileError("latexmk succeeded but expected PDF was not found")
    size = pdf.stat().st_size
    if size > max_bytes:
        raise CompileError("PDF exceeds configured maximum build artifact size")

    digest = hashlib.sha256(pdf.read_bytes()).hexdigest()
    return {
        "status": "success",
        "main_file": main_file,
        "pdf_path": "build/" + pdf.name,
        "pdf_bytes": size,
        "sha256": digest,
        "compiler": latexmk,
        "log_tail": log,
    }
