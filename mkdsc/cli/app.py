import platform
import subprocess
import time
from datetime import datetime

from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.prompt import Confirm, Prompt
from rich.table import Table
from rich.progress import Progress, SpinnerColumn, TextColumn

from mkdsc.constants import VERSION
from mkdsc.config import load_config, save_config
from mkdsc.devices import list_devices, save_device, remove_device
from mkdsc.i18n import get_translator
from mkdsc.i18n.lexicon_cli import LEXICON_CLI
from mkdsc.logging_utils import init_logging
from mkdsc.tools import (
    delete_setting,
    ensure_tools,
    get_connected_devices,
    get_device_info,
    get_device_wifi_ip,
    get_setting,
    put_setting,
    run_cmd,
    start_adb_server,
    stop_adb_server,
)
from mkdsc.updater import apply_update, check_for_updates

console = Console()


def _check_updates(t, logger, manual=False):
    console.print(f"[dim]{t('checking_updates')}[/dim]")
    try:
        info = check_for_updates()
    except Exception as exc:
        logger.info("Update check failed: %s", exc)
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
            result = apply_update(release, logger=logger)
            if result.get("success"):
                console.print(f"[green]{t('update_success')}[/green]")
                console.print(f"[dim]{t('update_restart')}[/dim]")
                return True
            console.print(f"[red]{t('update_failed')}[/red]")
            return False
    elif manual:
        console.print(f"[green]{t('update_latest')}[/green]")

    return False


def _render_devices_table(devices, title):
    table = Table(title=title, box=box.ROUNDED, show_header=True, header_style="bold cyan")
    table.add_column("#", style="dim", width=4)
    table.add_column("Type", width=8)
    table.add_column("Name", style="bold green")
    table.add_column("Address", style="yellow")
    table.add_column("Last used", style="dim")

    for index, device in enumerate(devices, 1):
        last_used = device.get("last_used")
        if last_used:
            last_used = datetime.fromisoformat(last_used).strftime("%d.%m.%Y %H:%M")
        else:
            last_used = "-"
        connection_icon = "Wi-Fi" if device.get("connection_type") == "wifi" else "USB"

        table.add_row(
            str(index),
            connection_icon,
            device.get("name", "-"),
            f"{device.get('ip')}:{device.get('port')}",
            last_used,
        )

    console.print(table)


def _quick_connect(adb_path, t):
    devices = list_devices()
    if not devices:
        console.print(f"[red]{t('no_saved_devices')}[/red]")
        return None, None

    _render_devices_table(devices, t("select_saved_device"))
    choices = [str(i) for i in range(len(devices) + 1)]
    choice = Prompt.ask(t("prompt_choice"), choices=choices, default="0")

    if choice == "0":
        return None, None

    device = devices[int(choice) - 1]
    address = f"{device['ip']}:{device['port']}"
    result = run_cmd([str(adb_path), "connect", address], show_output=False)

    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print(f"[red]{t('connect_failed')}[/red]")
        if Confirm.ask(t("retry_prompt"), default=False):
            return _quick_connect(adb_path, t)
        if Confirm.ask(t("remove_saved_prompt"), default=False):
            remove_device(device["ip"], device["port"])
        return None, None

    console.print(f"[green]{t('connect_success')}[/green]")
    save_device(device["name"], device["ip"], device["port"], device.get("connection_type", "wifi"))
    return device, device.get("connection_type", "wifi")


def _usb_direct(adb_path, t):
    devices = get_connected_devices(adb_path)
    authorized = [d for d in devices if d["status"] == "device"]
    unauthorized = [d for d in devices if d["status"] == "unauthorized"]

    if unauthorized:
        console.print(f"[yellow]{t('unauthorized_devices')}[/yellow]")
        return None

    if not authorized:
        console.print(f"[red]{t('no_devices')}[/red]")
        return None

    return authorized[0]


def _wireless_pairing(adb_path, t):
    console.print(Panel.fit(t("pair_steps_title"), border_style="cyan"))

    pair_address = Prompt.ask(t("pair_prompt_address"))
    pair_code = Prompt.ask(t("pair_prompt_code"))

    if ":" not in pair_address or len(pair_code) != 6 or not pair_code.isdigit():
        console.print(f"[red]{t('pair_failed')}[/red]")
        return None

    proc = subprocess.Popen(
        [str(adb_path), "pair", pair_address],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output, _ = proc.communicate(input=pair_code + "\n")
    if "Successfully paired" not in output:
        console.print(f"[red]{t('pair_failed')}[/red]")
        return None

    console.print(f"[green]{t('pair_success')}[/green]")

    connect_address = Prompt.ask(t("enter_ip_prompt"))
    result = run_cmd([str(adb_path), "connect", connect_address], show_output=False)
    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print(f"[red]{t('connect_failed')}[/red]")
        return None

    ip, port = connect_address.split(":")
    name = Prompt.ask(t("device_name_prompt"), default="")
    if name:
        save_device(name, ip, port, "wifi")

    return {"serial": connect_address, "status": "device"}


def _usb_to_wifi(adb_path, t):
    devices = get_connected_devices(adb_path)
    authorized = [d for d in devices if d["status"] == "device"]

    if not authorized:
        console.print(f"[red]{t('no_devices')}[/red]")
        return None

    port = Prompt.ask(t("tcpip_port_prompt"), default="5555")
    result = run_cmd([str(adb_path), "tcpip", port], show_output=False)

    if result.returncode != 0:
        console.print(f"[red]{t('tcpip_enable_failed')}[/red]")
        return None

    console.print(f"[green]{t('tcpip_enable_ok')}[/green]")

    ip_address = get_device_wifi_ip(adb_path) or Prompt.ask(t("enter_ip_prompt"))
    if not ip_address:
        return None

    result = run_cmd([str(adb_path), "connect", f"{ip_address}:{port}"], show_output=False)
    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print(f"[red]{t('connect_failed')}[/red]")
        return None

    name = Prompt.ask(t("device_name_prompt"), default="")
    if name:
        save_device(name, ip_address, port, "wifi")

    return {"serial": f"{ip_address}:{port}", "status": "device"}


def _select_connection(adb_path, t, logger):
    while True:
        table = Table(box=box.ROUNDED, show_header=False, title=t("connection_menu_title"))
        table.add_column("#", style="cyan", width=4)
        table.add_column("Option", style="white")
        table.add_row("1", t("connection_quick"))
        table.add_row("2", t("connection_usb"))
        table.add_row("3", t("connection_pair"))
        table.add_row("4", t("connection_usb_to_wifi"))
        table.add_row("5", t("connection_update"))
        table.add_row("0", t("connection_back"))
        console.print(table)

        choice = Prompt.ask(t("prompt_choice"), choices=["0", "1", "2", "3", "4", "5"], default="1")

        if choice == "0":
            return None, None
        if choice == "1":
            device, conn_type = _quick_connect(adb_path, t)
            if device:
                return device, conn_type
        elif choice == "2":
            device = _usb_direct(adb_path, t)
            if device:
                return device, "usb"
        elif choice == "3":
            device = _wireless_pairing(adb_path, t)
            if device:
                return device, "wifi"
        elif choice == "4":
            device = _usb_to_wifi(adb_path, t)
            if device:
                return device, "wifi"
        elif choice == "5":
            if _check_updates(t, logger, manual=True):
                return None, None


def _prompt_scrcpy_settings(config, t):
    presets = config.get("scrcpy", {}).get("presets", [])
    preset_names = [preset.get("name") for preset in presets]

    preset_choices = {"0": t("preset_custom")}
    for index, name in enumerate(preset_names, 1):
        preset_choices[str(index)] = name

    table = Table(box=box.SIMPLE, show_header=False, title=t("scrcpy_settings_title"))
    for key, name in preset_choices.items():
        table.add_row(key, name)
    console.print(table)

    default_choice = "1" if presets else "0"
    choice = Prompt.ask(t("preset_prompt"), choices=list(preset_choices.keys()), default=default_choice)

    if choice == "0":
        bitrate = Prompt.ask(t("bitrate_prompt"), default=config["scrcpy"]["bitrate"])
        maxsize = Prompt.ask(t("maxsize_prompt"), default=config["scrcpy"]["maxsize"])
        preset_name = t("preset_custom")
    else:
        preset = presets[int(choice) - 1]
        bitrate = preset.get("bitrate", config["scrcpy"]["bitrate"])
        maxsize = preset.get("maxsize", config["scrcpy"]["maxsize"])
        preset_name = preset.get("name")

    keyboard = Prompt.ask(
        t("keyboard_prompt"),
        choices=["uhid", "sdk", "aoa"],
        default=config["scrcpy"]["keyboard"],
    )

    stay_awake = Confirm.ask(t("option_stay_awake"), default=config["scrcpy"]["stay_awake"])
    show_touches = Confirm.ask(t("option_show_touches"), default=config["scrcpy"]["show_touches"])
    turn_screen_off = Confirm.ask(t("option_turn_screen_off"), default=config["scrcpy"]["turn_screen_off"])
    fullscreen = Confirm.ask(t("option_fullscreen"), default=config["scrcpy"]["fullscreen"])
    no_audio = Confirm.ask(t("option_no_audio"), default=config["scrcpy"]["no_audio"])

    config["scrcpy"].update({
        "bitrate": bitrate,
        "maxsize": maxsize,
        "keyboard": keyboard,
        "stay_awake": stay_awake,
        "show_touches": show_touches,
        "turn_screen_off": turn_screen_off,
        "fullscreen": fullscreen,
        "no_audio": no_audio,
    })
    save_config(config)

    return {
        "preset": preset_name,
        "bitrate": bitrate,
        "maxsize": maxsize,
        "keyboard": keyboard,
        "stay_awake": stay_awake,
        "show_touches": show_touches,
        "turn_screen_off": turn_screen_off,
        "fullscreen": fullscreen,
        "no_audio": no_audio,
    }


def _apply_device_settings(adb_path, stay_awake, show_touches):
    restore = {}

    if stay_awake:
        previous = get_setting(adb_path, "global", "stay_on_while_plugged_in")
        restore["global:stay_on_while_plugged_in"] = previous
        put_setting(adb_path, "global", "stay_on_while_plugged_in", 3)

    if show_touches:
        previous = get_setting(adb_path, "system", "show_touches")
        restore["system:show_touches"] = previous
        put_setting(adb_path, "system", "show_touches", 1)

    return restore


def _restore_device_settings(adb_path, restore):
    for key, value in restore.items():
        namespace, setting = key.split(":", 1)
        if value is None:
            delete_setting(adb_path, namespace, setting)
        else:
            put_setting(adb_path, namespace, setting, value)


def _launch_scrcpy(adb_path, scrcpy_path, settings, connection_mode, logger, t):
    keyboard = settings["keyboard"]
    if keyboard == "aoa" and platform.system().lower().startswith("win"):
        console.print(f"[yellow]{t('keyboard_aoa_windows_warning')}[/yellow]")
        keyboard = "uhid"

    cmd = [
        str(scrcpy_path),
        "--video-bit-rate", settings["bitrate"],
        "--max-size", settings["maxsize"],
        f"--keyboard={keyboard}",
    ]

    if connection_mode == "usb":
        cmd.append("--select-usb")
    elif connection_mode == "wifi":
        cmd.append("--select-tcpip")

    if settings["turn_screen_off"]:
        cmd.append("--turn-screen-off")
    if settings["fullscreen"]:
        cmd.append("--fullscreen")
    if settings["no_audio"]:
        cmd.append("--no-audio")

    restore = _apply_device_settings(adb_path, settings["stay_awake"], settings["show_touches"])

    logger.info("scrcpy settings: %s", settings)
    logger.info("scrcpy command: %s", " ".join(cmd))

    console.print(f"[cyan]{t('launching_scrcpy')}[/cyan]")
    try:
        run_cmd(cmd, show_output=True)
    finally:
        if restore:
            _restore_device_settings(adb_path, restore)
        console.print(f"[dim]{t('scrcpy_exit')}[/dim]")


def run_cli():
    config = load_config()
    t = get_translator(LEXICON_CLI, config.get("language", "en"))

    logger, _ = init_logging("cli")
    logger.info("version: %s", VERSION)
    logger.info("start: %s", datetime.now().isoformat())

    console.print(Panel.fit(
        f"[bold cyan]{t('app_title')}[/bold cyan]\n[dim]{t('app_subtitle')}[/dim]",
        border_style="cyan",
    ))

    if _check_updates(t, logger):
        return

    adb_path = None
    try:
        with Progress(SpinnerColumn(), TextColumn("{task.description}"), console=console) as progress:
            progress.add_task(t("tools_check"), total=None)
            adb_path, scrcpy_path = ensure_tools()

        start_adb_server(adb_path)
        console.print(f"[green]{t('adb_path')}:[/green] [dim]{adb_path}[/dim]")
        console.print(f"[green]{t('scrcpy_path')}:[/green] [dim]{scrcpy_path}[/dim]")

        device, connection_mode = _select_connection(adb_path, t, logger)
        if not device:
            return

        device_info = get_device_info(adb_path)
        local_ip = get_device_wifi_ip(adb_path)
        logger.info("connection_type: %s", connection_mode)
        logger.info("device_info: %s", device_info)
        logger.info("local_ip: %s", local_ip)

        settings = _prompt_scrcpy_settings(config, t)

        _launch_scrcpy(adb_path, scrcpy_path, settings, connection_mode, logger, t)

        time.sleep(1)
    except KeyboardInterrupt:
        console.print(f"\n[dim]{t('exit_message')}[/dim]")
    finally:
        if adb_path:
            stop_adb_server(adb_path)
