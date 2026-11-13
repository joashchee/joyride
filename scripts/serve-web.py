#!/usr/bin/env python3
"""Serves a love.js page folder for local testing, with the two headers the
standard (threaded) build needs, plus the right type for .wasm.

  scripts/serve-web.py <folder> [port]      # default port 8000

The compatibility build works without the headers; the standard one throws
"SharedArrayBuffer is not defined" without them.
"""
import http.server, sys, functools

class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}
    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

if __name__ == "__main__":
    folder = sys.argv[1] if len(sys.argv) > 1 else "."
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 8000
    handler = functools.partial(Handler, directory=folder)
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), handler) as srv:
        print(f"serving {folder} at http://127.0.0.1:{port}/ (Ctrl-C to stop)")
        srv.serve_forever()
