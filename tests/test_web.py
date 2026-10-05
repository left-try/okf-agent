import tempfile
import threading
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import urlopen
from unittest.mock import patch

from okf_agent.docs import build_docs
from okf_agent.knowledge import initialize
from okf_agent.web import _resolve_page, create_server


class WebTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        initialize(self.root)
        overview = self.root / ".okf" / "application" / "overview.md"
        overview.write_text(overview.read_text() + "\nDistinctive wiki search phrase.\n")
        build_docs(self.root)
        self.server = create_server(self.root)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base_url = f"http://127.0.0.1:{self.server.server_address[1]}"

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)
        self.temp.cleanup()

    def test_homepage_lists_pages_and_source_links(self):
        with urlopen(self.base_url + "/") as response:
            body = response.read().decode()
        self.assertIn("Repository Wiki", body)
        self.assertIn("source/.okf/application/overview.md", body)

    def test_search_finds_document_text(self):
        with urlopen(self.base_url + "/search?q=Distinctive+wiki") as response:
            body = response.read().decode()
        self.assertIn("Application Overview", body)

    def test_raw_html_in_markdown_is_escaped(self):
        overview = self.root / ".okf" / "application" / "overview.md"
        overview.write_text(overview.read_text() + "\n<script>alert('x')</script>\n")
        build_docs(self.root)

        with urlopen(self.base_url + "/source/.okf/application/overview.md") as response:
            body = response.read().decode()
        self.assertNotIn("<script>", body)
        self.assertIn("&lt;script&gt;", body)

    def test_unknown_and_traversal_paths_return_404(self):
        for path in ("/missing.md", "/%2e%2e/README.md"):
            with self.subTest(path=path):
                with self.assertRaises(HTTPError) as error:
                    urlopen(self.base_url + path)
                self.assertEqual(error.exception.code, 404)

    def test_page_resolver_rejects_a_path_resolving_outside_output(self):
        output = self.root / "isolated-site"
        page = output / "source" / ".okf" / "application" / "overview.md"
        page.parent.mkdir(parents=True)
        page.write_text("doc")
        outside = self.root / "secret.md"
        outside.write_text("secret")
        real_resolve = Path.resolve

        def resolve(path, *args, **kwargs):
            if path == page:
                return outside
            return real_resolve(path, *args, **kwargs)

        with patch.object(Path, "resolve", new=resolve):
            self.assertIsNone(_resolve_page(output, "/source/.okf/application/overview.md", {"source/.okf/application/overview.md"}))

    def test_symlink_escape_is_not_served(self):
        outside = self.root.parent / "outside-secret.md"
        outside.write_text("secret")
        source = self.root / ".okf-index" / "site" / "source" / ".okf" / "application" / "overview.md"
        original = source.read_bytes()
        source.unlink()
        try:
            source.symlink_to(outside)
        except OSError:
            source.write_bytes(original)
            self.skipTest("symlink creation is not available on this platform")
        try:
            with self.assertRaises(HTTPError) as error:
                urlopen(self.base_url + "/source/.okf/application/overview.md")
            self.assertEqual(error.exception.code, 404)
        finally:
            source.unlink(missing_ok=True)
            source.write_bytes(original)
            outside.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
