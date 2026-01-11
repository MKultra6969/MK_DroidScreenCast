from mkdsc.bootstrap import ensure_runtime
from mkdsc.app import run_app


def main():
    ensure_runtime()
    run_app()


if __name__ == "__main__":
    main()
