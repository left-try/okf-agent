from __future__ import annotations

import os
import re
import shutil
import tempfile
from pathlib import Path
from urllib.parse import quote

from .discovery import SOURCE_SUFFIXES, build_index, detect, files, indexed_content
from .knowledge import GENERATED_END, GENERATED_START


def _relative_link(output: Path, target: Path, label: str) -> str:
    relative = Path(os.path.relpath(target, output)).as_posix()
    destination = quote(relative, safe="/._-~")
    safe_label = label.replace("\\", "\\\\").replace("[", "\\[").replace("]", "\\]")
    return f"[{safe_label}]({destination})"


def _curated_overview(root: Path) -> str:
    overview = root / ".okf" / "application" / "overview.md"
    if not overview.is_file():
        return ""
    content = overview.read_text(encoding="utf-8", errors="replace")
    marker_pattern = re.escape(GENERATED_START) + r".*?" + re.escape(GENERATED_END)
    content = re.sub(marker_pattern, "", content, flags=re.S).strip()
    content = re.sub(r"^# Application Overview\s*", "", content, count=1)
    return content.strip()


def _render_pages(root: Path, indexed: dict[str, list[dict[str, str]]], output: Path) -> dict[str, str]:
    facts = detect(root)
    documents = indexed["documents"]
    source_documents = [item for item in documents if Path(item["path"]).suffix.lower() in SOURCE_SUFFIXES]
    symbols_by_path: dict[str, list[str]] = {}
    for item in indexed["symbols"]:
        symbols_by_path.setdefault(item["path"], []).append(item["name"])

    overview_path = root / ".okf" / "application" / "overview.md"
    overview = _curated_overview(root)
    if overview_path.is_file():
        overview_link = _relative_link(output, overview_path, ".okf/application/overview.md")
        overview_section = f"See {overview_link} for the source overview."
        if overview:
            overview_section += f"\n\n{overview}"
    else:
        overview_section = "No curated application overview has been added yet."
    languages = ", ".join(facts["languages"]) or "unknown"
    manifests = ", ".join(f"`{name}`" for name in facts["manifests"]) or "none detected"
    index_body = (
        "## Curated application overview\n\n"
        f"{overview_section}\n\n"
        "## Detected repository facts\n\n"
        f"- Languages detected from recognized manifests: {languages}\n"
        f"- Recognized manifests: {manifests}\n"
        f"- Indexed source files: {len(source_documents)}\n\n"
        "## Code reference\n\n"
        "- [Indexed files, symbols, and import edges](code-map.md)\n"
    )

    file_lines = ["## Indexed source files", ""]
    if source_documents:
        for document in source_documents:
            relative = document["path"]
            link = _relative_link(output, root / Path(relative), relative)
            file_lines.append(f"- {link}")
    else:
        file_lines.append("No indexed files or symbols were found.")

    symbol_lines = ["## Indexed symbols", ""]
    if indexed["symbols"]:
        for path in sorted(symbols_by_path):
            names = sorted(symbols_by_path[path])
            link = _relative_link(output, root / Path(path), path)
            symbol_lines.append(f"- {link}: " + ", ".join(f"`{name}`" for name in names))
    else:
        symbol_lines.append("No indexed files or symbols were found.")

    edge_lines = ["## Indexed import edges", ""]
    if indexed["edges"]:
        for edge in indexed["edges"]:
            source = edge["source"]
            link = _relative_link(output, root / Path(source), source)
            edge_lines.append(f"- {link} → `{edge['target']}` ({edge['kind']})")
    else:
        edge_lines.append("No import edges were found in the index.")

    code_map_body = "\n".join(file_lines + [""] + symbol_lines + [""] + edge_lines)
    return {
        "index.md": f"# Project Documentation\n\n{GENERATED_START}\n{index_body}{GENERATED_END}\n",
        "code-map.md": f"# Code Map\n\n{GENERATED_START}\n{code_map_body}\n{GENERATED_END}\n",
    }


def _output_directory(root: Path, output: Path | None) -> Path:
    root = root.resolve()
    candidate = output if output is not None else Path("docs") / "reference"
    if not candidate.is_absolute():
        candidate = root / candidate
    candidate = Path(os.path.abspath(candidate))
    try:
        relative = candidate.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"Documentation output must stay inside the repository: {candidate}") from exc
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"Documentation output cannot traverse a symlink: {current}")
    resolved = candidate.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"Documentation output must stay inside the repository: {candidate}") from exc
    return resolved


def _managed_target(root: Path, output: Path, name: str) -> Path:
    target = output / name
    if target.is_symlink():
        raise ValueError(f"Generated documentation page cannot be a symlink: {target}")
    resolved = target.resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise ValueError(f"Generated documentation page must stay inside the repository: {target}") from exc
    return target


def _generated_block(text: str) -> str | None:
    starts = text.count(GENERATED_START)
    ends = text.count(GENERATED_END)
    if starts != 1 or ends != 1:
        return None
    start = text.index(GENERATED_START) + len(GENERATED_START)
    end = text.index(GENERATED_END)
    if end < start:
        return None
    return text[start:end].strip()


def _merge_generated(existing: str | None, rendered: str) -> str:
    if existing is None:
        return rendered
    expected = _generated_block(rendered)
    if expected is None:
        raise ValueError("Internal docs renderer produced an invalid generated block")
    current = _generated_block(existing)
    if current is None:
        if GENERATED_START in existing or GENERATED_END in existing:
            raise ValueError("Existing documentation has malformed OKF generated markers")
        separator = "\n" if existing.endswith("\n") else "\n\n"
        start = rendered.index(GENERATED_START)
        return existing.rstrip() + separator + rendered[start:]
    start = existing.index(GENERATED_START) + len(GENERATED_START)
    end = existing.index(GENERATED_END)
    replacement_start = rendered.index(GENERATED_START) + len(GENERATED_START)
    replacement_end = rendered.index(GENERATED_END)
    return existing[:start] + rendered[replacement_start:replacement_end] + existing[end:]


def _render_expected(root: Path, output: Path) -> dict[str, str]:
    return _render_pages(root, indexed_content(root), output)


def build_docs(root: Path, output: Path | None = None) -> dict[str, object]:
    """Build the deterministic, index-backed human reference pages on request."""
    root = root.resolve()
    destination = _output_directory(root, output)
    build_index(root)
    rendered = _render_expected(root, destination)
    updated: list[str] = []
    page_paths: list[str] = []
    for name, template in sorted(rendered.items()):
        target = _managed_target(root, destination, name)
        page_paths.append(target.relative_to(root).as_posix())
        before = target.read_text(encoding="utf-8") if target.is_file() else None
        after = _merge_generated(before, template)
        if before != after:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(after, encoding="utf-8", newline="\n")
            updated.append(target.relative_to(root).as_posix())
    return {"output": destination.relative_to(root).as_posix(), "pages": page_paths, "updated": updated}


def check_docs(root: Path, output: Path | None = None) -> dict[str, object]:
    """Check generated pages against a fresh disposable index without writing docs."""
    root = root.resolve()
    destination = _output_directory(root, output)
    with tempfile.TemporaryDirectory(prefix="okf-docs-index-") as directory:
        temp_root = Path(directory)
        for source in files(root):
            relative = source.relative_to(root)
            target = temp_root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        build_index(temp_root)
        indexed = indexed_content(temp_root)
        rendered = _render_pages(root, indexed, destination)

    stale: list[str] = []
    errors: list[str] = []
    for name, template in sorted(rendered.items()):
        target = _managed_target(root, destination, name)
        relative = target.relative_to(root).as_posix()
        if not target.is_file():
            stale.append(relative)
            errors.append(f"Missing generated documentation: {relative}; run `okf-agent docs build`.")
            continue
        actual = _generated_block(target.read_text(encoding="utf-8"))
        expected = _generated_block(template)
        if actual is None:
            stale.append(relative)
            errors.append(f"Malformed generated block in {relative}; repair it or run `okf-agent docs build`.")
        elif actual != expected:
            stale.append(relative)
            errors.append(f"Generated documentation is stale: {relative}; run `okf-agent docs build`.")
    return {"ok": not errors, "stale": stale, "errors": errors}
