import os

from mkdsc.web.server import run_server


def main():
    host = os.environ.get("MKDSC_HOST", "127.0.0.1")
    port = int(os.environ.get("MKDSC_PORT", "6969"))
    auto_open_raw = os.environ.get("MKDSC_AUTO_OPEN", "0").strip().lower()
    auto_open = auto_open_raw in {"1", "true", "yes", "on"}
    run_server(host=host, port=port, auto_open=auto_open)


if __name__ == "__main__":
    main()
