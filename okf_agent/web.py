from __future__ import annotations

import html
import json
import re
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from urllib.parse import parse_qs, quote, unquote, urlsplit

from .docs import MANIFEST, _output_path, _title, build_docs, check_docs


LINK = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
INLINE_CODE = re.compile(r"`([^`]+)`")


def _safe_href(href: str, current: str, allowed: set[str]) -> str:
    parsed = urlsplit(href)
    if parsed.scheme in {"http", "https", "mailto"}:
        return href
    if parsed.scheme or parsed.netloc:
        return "#"
    if not parsed.path:
        return "#" + quote(parsed.fragment) if parsed.fragment else "#"
    decoded = unquote(parsed.path)
    if "\\" in decoded or "\x00" in decoded:
        return "#"
    base = PurePosixPath(current).parent
    target = PurePosixPath(decoded.lstrip("/")) if decoded.startswith("/") else base / decoded
    normalized = PurePosixPath()
    parts: list[str] = []
    for part in target.parts:
        if part in {"", "."}:
            continue
        if part == "..":
            if not parts:
                return "#"
            parts.pop()
        else:
            parts.append(part)
    route = PurePosixPath(*parts).as_posix()
    if route not in allowed or not route.endswith(".md"):
        return "#"
    result = "/" + quote(route, safe="/")
    if parsed.fragment:
        result += "#" + quote(parsed.fragment)
    return result


def _inline(text: str, current: str, allowed: set[str]) -> str:
    pieces: list[str] = []
    position = 0
    for match in LINK.finditer(text):
        pieces.append(html.escape(text[position:match.start()]))
        label, href = match.groups()
        safe_href = html.escape(_safe_href(href, current, allowed), quote=True)
        pieces.append(f'<a href="{safe_href}">{html.escape(label)}</a>')
        position = match.end()
    pieces.append(html.escape(text[position:]))
    escaped = "".join(pieces)
    return INLINE_CODE.sub(lambda match: f"<code>{match.group(1)}</code>", escaped)


def _render_markdown(text: str, current: str, allowed: set[str]) -> str:
    output: list[str] = []
    paragraph: list[str] = []
    in_list = False
    in_code = False
    code_lines: list[str] = []

    def flush_paragraph() -> None:
        if paragraph:
            output.append("<p>" + " ".join(_inline(line, current, allowed) for line in paragraph) + "</p>")
            paragraph.clear()

    def close_list() -> None:
        nonlocal in_list
        if in_list:
            output.append("</ul>")
            in_list = False

    for line in text.splitlines():
        if line.strip().startswith("```"):
            flush_paragraph()
            close_list()
            if in_code:
                output.append("<pre><code>" + html.escape("\n".join(code_lines)) + "</code></pre>")
                code_lines.clear()
            in_code = not in_code
            continue
        if in_code:
            code_lines.append(line)
            continue
        if not line.strip():
            flush_paragraph()
            close_list()
            continue
        heading = re.match(r"^(#{1,6})\s+(.+)$", line)
        if heading:
            flush_paragraph()
            close_list()
            level = len(heading.group(1))
            output.append(f"<h{level}>{_inline(heading.group(2), current, allowed)}</h{level}>")
            continue
        item = re.match(r"^\s*[-*+]\s+(.+)$", line)
        if item:
            flush_paragraph()
            if not in_list:
                output.append("<ul>")
                in_list = True
            output.append("<li>" + _inline(item.group(1), current, allowed) + "</li>")
            continue
        close_list()
        paragraph.append(line)

    flush_paragraph()
    close_list()
    if in_code:
        output.append("<pre><code>" + html.escape("\n".join(code_lines)) + "</code></pre>")
    return "\n".join(output)


def _resolve_page(output: Path, route: str, allowed: set[str]) -> tuple[str, Path] | None:
    decoded = unquote(route)
    if "\\" in decoded or "\x00" in decoded:
        return None
    if decoded in {"", "/"}:
        decoded = "index.md"
    else:
        decoded = decoded.lstrip("/")
    parts = PurePosixPath(decoded).parts
    if any(part in {"..", "."} for part in parts) or decoded not in allowed or not decoded.endswith(".md"):
        return None
    path = (output / Path(*parts)).resolve()
    try:
        path.relative_to(output.resolve())
    except ValueError:
        return None
    cursor = output
    for part in parts:
        cursor = cursor / part
        if cursor.is_symlink():
            return None
    if not path.is_file():
        return None
    return decoded, path


def create_server(root: Path, host: str = "127.0.0.1", port: int = 0) -> ThreadingHTTPServer:
    root = root.resolve()
    output = _output_path(root, None)
    if not check_docs(root)["ok"]:
        build_docs(root)
    manifest = json.loads((output / MANIFEST).read_text(encoding="utf-8"))
    allowed = {name for name in manifest.get("outputs", {}) if isinstance(name, str)}

    class Handler(BaseHTTPRequestHandler):
        def _response(self, status: int, body: str) -> None:
            data = body.encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'")
            self.end_headers()
            self.wfile.write(data)

        def _wrapper(self, title: str, content: str) -> str:
            return (
                "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">"
                f"<title>{html.escape(title)}</title><style>body{{font:16px/1.55 system-ui,sans-serif;max-width:900px;margin:2rem auto;padding:0 1rem;color:#222}}"
                "a{color:#0759b5}pre{overflow:auto;background:#f5f5f5;padding:1rem}code{background:#f5f5f5;padding:.1rem .25rem}nav{border-bottom:1px solid #ddd;padding-bottom:1rem;margin-bottom:2rem}</style></head><body>"
                '<nav><a href="/">Repository wiki</a> · <a href="/repository.md">Project facts</a>'
                '<form action="/search" method="get" style="display:inline;margin-left:1rem">'
                '<input name="q" aria-label="Search docs"><button>Search</button></form></nav>'
                f"<main>{content}</main></body></html>"
            )

        def do_GET(self) -> None:
            parsed = urlsplit(self.path)
            if parsed.path == "/search":
                query = parse_qs(parsed.query).get("q", [""])[0].strip()
                if not query:
                    self._response(200, self._wrapper("Search", "<h1>Search</h1><p>Enter a search term.</p>"))
                    return
                results: list[str] = []
                if len(query) <= 200:
                    for name in sorted(allowed):
                        resolved = _resolve_page(output, "/" + name, allowed)
                        if not resolved:
                            continue
                        _, path = resolved
                        text = path.read_text(encoding="utf-8", errors="replace")
                        if query.casefold() in text.casefold():
                            title = _title(path, text)
                            href = "/" + quote(name, safe="/")
                            results.append(f'<li><a href="{html.escape(href, quote=True)}">{html.escape(title)}</a></li>')
                content = f"<h1>Search results</h1><p>Query: {html.escape(query)}</p>" + ("<ul>" + "".join(results) + "</ul>" if results else "<p>No matching pages.</p>")
                self._response(200, self._wrapper("Search", content))
                return

            resolved = _resolve_page(output, parsed.path, allowed)
            if not resolved:
                self._response(404, self._wrapper("Not found", "<h1>Page not found</h1>"))
                return
            name, path = resolved
            text = path.read_text(encoding="utf-8", errors="replace")
            rendered = _render_markdown(text, name, allowed)
            self._response(200, self._wrapper(_title(path, text), rendered))

        def log_message(self, format: str, *args: object) -> None:
            return

    return ThreadingHTTPServer((host, port), Handler)


def serve_docs(root: Path, host: str = "127.0.0.1", port: int = 0) -> None:
    """Build and serve local documentation, bound to loopback by default."""
    server = create_server(root, host=host, port=port)
    print(f"OKF documentation: http://{server.server_address[0]}:{server.server_address[1]}/")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
