import sys

from rich import box

from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.table import Table

from mkdsc.cli.app import run_cli
from mkdsc.config import load_config
from mkdsc.i18n import get_translator
from mkdsc.i18n.lexicon_cli import LEXICON_CLI
from mkdsc.updater import apply_update, check_for_updates
from mkdsc.web.server import run_server

console = Console()


def _check_updates(t, manual=False):
    console.print(f"[dim]{t('checking_updates')}[/dim]")
    try:
        info = check_for_updates()
    except Exception:
        console.print(f"[yellow]{t('update_check_failed')}[/yellow]")
        return False

    latest = info.get("latest")
    if info.get("update_available"):
        release = info.get("release", {})
        console.print(Panel.fit(
            f"{t('update_available', latest=latest)}\n\n{release.get('body', '').strip()}",
            title="Update",
            border_style="yellow",
        ))
        if Confirm.ask(t("update_prompt"), default=False):
            result = apply_update(release)
            if result.get("success"):
                console.print(f"[green]{t('update_success')}[/green]")
                console.print(f"[dim]{t('update_restart')}[/dim]")
                return True
            console.print(f"[red]{t('update_failed')}[/red]")
            return False
    elif manual:
        console.print(f"[green]{t('update_latest')}[/green]")

    return False


def run_app():
    config = load_config()
    t = get_translator(LEXICON_CLI, config.get("language", "en"))

    console.print(Panel.fit(
        f"[bold cyan]{t('app_title')}[/bold cyan]\n[dim]{t('app_subtitle')}[/dim]",
        border_style="cyan",
    ))

    if _check_updates(t):
        return

    while True:
        table = Table(box=box.ROUNDED, show_header=False, title=t("menu_title"))
        table.add_column("#", style="cyan", width=4)
        table.add_column("Option", style="white")

        table.add_row("1", t("menu_web"))
        table.add_row("2", t("menu_cli"))
        table.add_row("3", t("menu_update_check"))
        table.add_row("0", t("menu_exit"))

        console.print(table)

        choice = Prompt.ask(t("prompt_choice"), choices=["0", "1", "2", "3"], default="1")

        if choice == "1":
            run_server()
            return
        if choice == "2":
            run_cli()
            return
        if choice == "3":
            if _check_updates(t, manual=True):
                return
            continue

        console.print(f"\n[dim]{t('exit_message')}[/dim]")
        sys.exit(0)


if __name__ == "__main__":
    run_app()
