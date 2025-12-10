import os
import sys
import zipfile
import shutil
import subprocess
import requests
import time
import json
from pathlib import Path
from datetime import datetime
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.prompt import Prompt, Confirm
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich import box
from rich.text import Text

console = Console()

def retry_or_menu(func):
    def wrapper(*args, **kwargs):
        while True:
            try:
                result = func(*args, **kwargs)
                if result is False:
                    if not Confirm.ask("\n[yellow]Попробовать снова?[/yellow]", default=True):
                        return False
                    console.print("[cyan]Попробуем ещё раз...[/cyan]\n")
                    continue
                return result
            except Exception as e:
                console.print(f"[red]❌ Ошибка: {e}[/red]")
                if not Confirm.ask("\n[yellow]Попробовать снова?[/yellow]", default=True):
                    return False
    return wrapper

BASE_DIR = Path(__file__).parent.resolve()
TOOLS_DIR = BASE_DIR / "downloads"
CONFIG_FILE = BASE_DIR / "devices.json"
TOOLS_DIR.mkdir(exist_ok=True)


URLS = {
    "platform-tools": "https://dl.google.com/android/repository/platform-tools-latest-windows.zip",
    "scrcpy": "https://github.com/Genymobile/scrcpy/releases/download/v3.3.3/scrcpy-win64-v3.3.3.zip"
}


def load_devices():
    if CONFIG_FILE.exists():
        try:
            with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
                return json.load(f)
        except:
            return {"devices": []}
    return {"devices": []}


def save_device(name, ip, port, connection_type):
    config = load_devices()
    
    for device in config["devices"]:
        if device["ip"] == ip and device["port"] == port:
            device["name"] = name
            device["connection_type"] = connection_type
            device["last_used"] = datetime.now().isoformat()
            break
    else:
        config["devices"].append({
            "name": name,
            "ip": ip,
            "port": port,
            "connection_type": connection_type,
            "last_used": datetime.now().isoformat()
        })
    
    with open(CONFIG_FILE, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=2, ensure_ascii=False)


def remove_device(ip, port):
    config = load_devices()
    config["devices"] = [d for d in config["devices"] 
                         if not (d["ip"] == ip and d["port"] == port)]
    
    with open(CONFIG_FILE, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=2, ensure_ascii=False)


def show_saved_devices():
    config = load_devices()
    devices = config.get("devices", [])
    
    if not devices:
        return None
    
    table = Table(title="📱 Сохраненные устройства", box=box.ROUNDED, show_header=True, header_style="bold cyan")
    table.add_column("#", style="dim", width=4)
    table.add_column("Тип", width=6)
    table.add_column("Имя", style="bold green")
    table.add_column("Адрес", style="yellow")
    table.add_column("Последнее использование", style="dim")
    
    for i, device in enumerate(devices, 1):
        last_used = datetime.fromisoformat(device["last_used"]).strftime("%d.%m.%Y %H:%M")
        connection_icon = "📡" if device["connection_type"] == "wifi" else "🔌"
        
        table.add_row(
            str(i),
            connection_icon,
            device['name'],
            f"{device['ip']}:{device['port']}",
            last_used
        )
    
    console.print(table)
    return devices


def quick_connect(adb_path):
    devices = show_saved_devices()
    
    if not devices:
        console.print("\n[red]❌ Нет сохраненных устройств[/red]")
        return False
    
    console.print("\n[dim]0. Назад[/dim]")
    choice = Prompt.ask(
        "\n[cyan]Выберите устройство[/cyan]", 
        choices=[str(i) for i in range(len(devices) + 1)], 
        default="0"
    )
    
    if choice == "0":
        return None
    
    idx = int(choice) - 1
    device = devices[idx]
    address = f"{device['ip']}:{device['port']}"
    
    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        console=console
    ) as progress:
        task = progress.add_task(f"Подключаюсь к {device['name']} ({address})...", total=None)
        result = run_cmd([str(adb_path), "connect", address], show_output=False)
    
    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print(Panel(
            "[red]❌ Не удалось подключиться![/red]\n\n"
            "[yellow]Возможные причины:[/yellow]\n"
            "• Устройство выключено или не в сети\n"
            "• Беспроводная отладка отключена\n"
            "• Изменился IP адрес устройства\n"
            "• Устройство и ПК в разных сетях",
            title="Ошибка подключения",
            border_style="red"
        ))
        
        if Confirm.ask("\nУдалить это устройство из списка?", default=False):
            remove_device(device['ip'], device['port'])
            console.print("[green]✅ Устройство удалено[/green]")
        
        return False
    
    console.print(f"[green]✅ Подключено к {device['name']}![/green]")
    save_device(device['name'], device['ip'], device['port'], device['connection_type'])
    
    return True


def download_and_extract(name, url):
    print(f"[+] Скачиваю {name}...")
    zip_path = TOOLS_DIR / f"{name}.zip"
    
    with requests.get(url, stream=True) as r:
        r.raise_for_status()
        with open(zip_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)
    
    print(f"[+] Распаковываю {name}...")
    with zipfile.ZipFile(zip_path, "r") as zf:
        zf.extractall(TOOLS_DIR)
    zip_path.unlink()


def ensure_tools():
    adb_path = next(TOOLS_DIR.glob("**/adb.exe"), None)
    scrcpy_path = next(TOOLS_DIR.glob("**/scrcpy.exe"), None)
    
    if not adb_path:
        download_and_extract("platform-tools", URLS["platform-tools"])
        adb_path = next(TOOLS_DIR.glob("**/adb.exe"), None)
        if not adb_path:
            raise FileNotFoundError("❌ adb.exe не найден после распаковки")
    
    if not scrcpy_path:
        download_and_extract("scrcpy", URLS["scrcpy"])
        scrcpy_path = next(TOOLS_DIR.glob("**/scrcpy.exe"), None)
        if not scrcpy_path:
            raise FileNotFoundError("❌ scrcpy.exe не найден после распаковки")
    
    return adb_path, scrcpy_path


def run_cmd(cmd, cwd=None, show_output=True):
    if show_output:
        print("> " + " ".join(map(str, cmd)))
    
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    
    if show_output:
        if result.stdout:
            print(result.stdout.strip())
        if result.stderr:
            print(result.stderr.strip(), file=sys.stderr)
    
    return result


def get_connected_devices(adb_path):
    result = run_cmd([str(adb_path), "devices"], show_output=False)
    lines = result.stdout.strip().split('\n')[1:]
    
    devices = []
    for line in lines:
        if '\t' in line:
            serial, status = line.split('\t')
            devices.append({'serial': serial, 'status': status})
    
    return devices


@retry_or_menu
def usb_direct(adb_path):
    console.print(Panel(
        "[cyan]Подключите телефон к ПК через USB кабель[/cyan]\n\n"
        "[yellow]Убедитесь что:[/yellow]\n"
        "• USB-отладка включена в параметрах разработчика\n"
        "• На телефоне разрешена отладка для этого ПК",
        title="📱 ПРЯМОЕ USB ПОДКЛЮЧЕНИЕ",
        border_style="cyan"
    ))
    
    Prompt.ask("\n[dim]Нажмите Enter когда подключите телефон[/dim]", default="")
    
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
        progress.add_task("Проверяю подключённые устройства...", total=None)
        devices = get_connected_devices(adb_path)
    
    if not devices:
        console.print(Panel(
            "[red]❌ ТЕЛЕФОН НЕ ОБНАРУЖЕН![/red]\n\n"
            "[yellow]🔧 Что проверить:[/yellow]\n"
            "1. USB-отладка включена\n"
            "2. На телефоне нажали 'Разрешить' при запросе отладки\n"
            "3. USB кабель исправен (попробуйте другой)\n"
            "4. Попробуйте другой USB порт на ПК\n"
            "5. Установлены драйверы для вашего телефона",
            border_style="red"
        ))
        return False
    
    unauthorized = [d for d in devices if d['status'] == 'unauthorized']
    if unauthorized:
        console.print(f"\n[yellow]⚠️  Обнаружено неавторизованных устройств: {len(unauthorized)}[/yellow]")
        console.print("\n[cyan]📋 На телефоне должен появиться запрос на разрешение отладки[/cyan]")
        console.print("Разрешите отладку и установите галочку 'Всегда разрешать с этого компьютера'")
        
        Prompt.ask("\n[dim]Нажмите Enter после разрешения[/dim]", default="")
        
        with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
            progress.add_task("Перезапускаю ADB сервер...", total=None)
            run_cmd([str(adb_path), "kill-server"], show_output=False)
            time.sleep(1)
            run_cmd([str(adb_path), "start-server"], show_output=False)
            time.sleep(1)
        
        devices = get_connected_devices(adb_path)
        unauthorized = [d for d in devices if d['status'] == 'unauthorized']
        
        if unauthorized:
            console.print("[red]❌ Устройство всё ещё не авторизовано![/red]")
            return False
    
    authorized_devices = [d for d in devices if d['status'] == 'device']
    
    if not authorized_devices:
        console.print("[red]❌ Нет авторизованных устройств![/red]")
        return False
    
    console.print(f"\n[green]✅ Обнаружено устройств: {len(authorized_devices)}[/green]")
    for dev in authorized_devices:
        console.print(f"   [yellow]📱 {dev['serial']}[/yellow]")
    
    return True


@retry_or_menu
def wireless_pairing(adb_path):
    console.print(Panel(
        "[cyan]📋 Инструкция на телефоне:[/cyan]\n"
        "1. Настройки → Система → Параметры разработчика\n"
        "2. Включите 'Беспроводная отладка' (Wireless Debugging)\n"
        "3. Нажмите 'Установить связь с устройством по коду'\n"
        "4. Появится окно с кодом и адресом для сопряжения\n\n"
        "[yellow]⚠️  Этот порт ТОЛЬКО для сопряжения![/yellow]",
        title="📱 БЕСПРОВОДНАЯ ОТЛАДКА (Android 11+)",
        border_style="cyan"
    ))
    
    Prompt.ask("\n[dim]Нажмите Enter когда увидите код и адрес[/dim]", default="")
    
    pair_address = Prompt.ask("\n[cyan]🔢 Введите адрес для сопряжения[/cyan] (IP:port)")
    if ":" not in pair_address:
        console.print("[red]❌ Неправильный формат![/red]")
        return False
    
    pair_code = Prompt.ask("[cyan]🔑 Введите 6-значный код[/cyan]")
    if len(pair_code) != 6 or not pair_code.isdigit():
        console.print("[red]❌ Код должен содержать 6 цифр![/red]")
        return False
    
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
        progress.add_task(f"Выполняю сопряжение с {pair_address}...", total=None)
        proc = subprocess.Popen(
            [str(adb_path), "pair", pair_address],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        output, _ = proc.communicate(input=pair_code + "\n")
    
    if "Successfully paired" not in output:
        console.print(f"[red]❌ Сопряжение не удалось![/red]\n[dim]{output}[/dim]")
        return False
    
    console.print("[green]✅ Сопряжение успешно![/green]")
    
    console.print(Panel(
        "[cyan]📋 На телефоне:[/cyan]\n"
        "1. Нажмите НАЗАД\n"
        "2. На главном экране найдите 'IP-адрес и порт' (сверху)\n"
        "3. Например: 192.168.1.23:41111\n\n"
        "[yellow]⚠️  НЕ используйте порт 5555![/yellow]",
        title="⚠️  ВАЖНО: Теперь нужен ДРУГОЙ порт!",
        border_style="yellow"
    ))
    
    Prompt.ask("\n[dim]Нажмите Enter когда найдёте адрес[/dim]", default="")
    
    connect_address = Prompt.ask("\n[cyan]🌐 Введите IP:порт с главного экрана[/cyan]")
    
    if ":" not in connect_address:
        console.print("[red]❌ Неправильный формат![/red]")
        return False
    
    if connect_address.endswith(":5555"):
        console.print("\n[yellow]⚠️  Вы ввели :5555 - это может не сработать для Android 11+[/yellow]")
        if not Confirm.ask("Продолжить?"):
            connect_address = Prompt.ask("\n[cyan]🌐 Введите правильный адрес[/cyan]")
    
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
        progress.add_task(f"Подключаюсь к {connect_address}...", total=None)
        result = run_cmd([str(adb_path), "connect", connect_address], show_output=False)
    
    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print("[red]❌ Подключение не удалось![/red]")
        return False
    
    console.print("[green]✅ Подключение установлено![/green]")
    
    ip, port = connect_address.split(":")
    device_name = Prompt.ask("\n[cyan]💾 Сохранить это устройство? Введите имя[/cyan] (или Enter для пропуска)", default="")
    
    if device_name:
        save_device(device_name, ip, port, "wifi")
        console.print(f"[green]✅ Устройство '{device_name}' сохранено![/green]")
    
    return True


@retry_or_menu
def usb_to_wireless(adb_path):
    console.print(Panel("Подключите телефон через USB", title="📱 USB → Wi-Fi", border_style="cyan"))
    
    Prompt.ask("\n[dim]Нажмите Enter после подключения[/dim]", default="")
    
    devices = get_connected_devices(adb_path)
    authorized = [d for d in devices if d['status'] == 'device']
    
    if not authorized:
        console.print("[red]❌ Телефон не обнаружен![/red]")
        return False
    
    port = Prompt.ask("\n[cyan]🔢 Порт для ADB[/cyan]", default="5555")
    
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
        progress.add_task(f"Переключаю в tcpip режим на порту {port}...", total=None)
        result = run_cmd([str(adb_path), "tcpip", port], show_output=False)
    
    if result.returncode != 0:
        console.print("[red]❌ Не удалось переключить режим![/red]")
        return False
    
    console.print("[green]✅ Режим tcpip активирован![/green]")
    
    result = run_cmd([str(adb_path), "shell", "ip", "addr", "show", "wlan0"], show_output=False)
    
    ip_address = None
    for line in result.stdout.split('\n'):
        if 'inet ' in line and 'inet6' not in line:
            parts = line.strip().split()
            if len(parts) >= 2:
                ip_address = parts[1].split('/')[0]
                break
    
    if ip_address:
        console.print(f"[yellow]📱 Обнаружен IP: {ip_address}[/yellow]")
        if not Confirm.ask("Использовать?", default=True):
            ip_address = None
    
    if not ip_address:
        ip_address = Prompt.ask("\n[cyan]🌐 Введите IP телефона[/cyan]")
    
    if not ip_address:
        return False
    
    console.print("\n[yellow]⚠️  Отключите USB кабель[/yellow]")
    Prompt.ask("[dim]Нажмите Enter после отключения[/dim]", default="")
    
    time.sleep(2)
    
    with Progress(SpinnerColumn(), TextColumn("[progress.description]{task.description}"), console=console) as progress:
        progress.add_task(f"Подключаюсь к {ip_address}:{port}...", total=None)
        result = run_cmd([str(adb_path), "connect", f"{ip_address}:{port}"], show_output=False)
    
    if result.returncode != 0 or "connected" not in result.stdout.lower():
        console.print("[red]❌ Подключение не удалось![/red]")
        return False
    
    console.print("[green]✅ Подключено![/green]")
    
    device_name = Prompt.ask("\n[cyan]💾 Сохранить устройство? Введите имя[/cyan] (или Enter для пропуска)", default="")
    
    if device_name:
        save_device(device_name, ip_address, port, "wifi")
        console.print(f"[green]✅ '{device_name}' сохранено![/green]")
    
    return True


def check_connection(adb_path):
    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        console=console
    ) as progress:
        progress.add_task("Проверяю подключение...", total=None)
        devices = get_connected_devices(adb_path)
    
    authorized = [d for d in devices if d['status'] == 'device']
    
    if not authorized:
        console.print("[red]❌ Нет подключенных устройств![/red]")
        return False
    
    table = Table(title=f"✅ Активных устройств: {len(authorized)}", box=box.SIMPLE, show_header=True, header_style="bold cyan")
    table.add_column("Тип", width=10)
    table.add_column("Serial", style="yellow")
    
    for dev in authorized:
        conn_type = "📡 Wi-Fi" if ":" in dev['serial'] else "🔌 USB"
        table.add_row(conn_type, dev['serial'])
    
    console.print(table)
    return True


def launch_scrcpy(scrcpy_path, connection_mode):
    print("\n" + "="*60)
    print("⚙️  НАСТРОЙКИ SCRCPY")
    print("="*60)
    
    bitrate = input("\n🎥 Битрейт видео [8M]: ").strip() or "8M"
    maxsize = input("📐 Макс. разрешение [1080]: ").strip() or "1080"
    
    print("\n📋 Режим клавиатуры:")
    print("1. UHID (рекомендуется, Android 9+, полная поддержка кириллицы)")
    print("2. SDK (по умолчанию, только ASCII)")
    print("3. AOA (физическая USB клавиатура)")
    
    keyboard_mode = input("\nВыберите режим [1]: ").strip() or "1"
    
    keyboard_flag = "--keyboard=uhid"
    if keyboard_mode == "2":
        keyboard_flag = "--keyboard=sdk"
    elif keyboard_mode == "3":
        keyboard_flag = "--keyboard=aoa"
    
    print("\n📋 Дополнительно:")
    stay_awake = input("⏰ Не выключать экран? (y/n) [n]: ").strip().lower() == 'y'
    turn_screen_off = input("🌑 Выключить экран телефона? (y/n) [n]: ").strip().lower() == 'y'
    show_touches = input("👆 Показывать нажатия? (y/n) [n]: ").strip().lower() == 'y'
    fullscreen = input("🖥️  Полный экран? (y/n) [n]: ").strip().lower() == 'y'
    no_audio = input("🔇 Без звука? (y/n) [n]: ").strip().lower() == 'y'
    
    cmd = [
        str(scrcpy_path),
        "--video-bit-rate", bitrate,
        "--max-size", maxsize,
        keyboard_flag
    ]
    
    if connection_mode == "usb":
        cmd.extend(["--select-usb"])
    elif connection_mode == "wifi":
        cmd.extend(["--select-tcpip"])
    
    if stay_awake:
        cmd.append("--stay-awake")
    if turn_screen_off:
        cmd.append("--turn-screen-off")
    if show_touches:
        cmd.append("--show-touches")
    if fullscreen:
        cmd.append("--fullscreen")
    if no_audio:
        cmd.append("--no-audio")
    
    if keyboard_mode == "1":
        cmd.append("--keyboard=uhid")
    
    print("\n" + "="*60)
    print("🚀 ЗАПУСКАЮ SCRCPY")
    print("="*60)
    
    if keyboard_mode == "1":
        print("\n✅ UHID режим активен:")
        print("   • Кириллица работает напрямую")
        print("   • Экранная клавиатура отключена")
        print("   • Переключай язык на ПК как обычно")
    else:
        print("\n⚠️  SDK режим - кириллица через Ctrl+V")
    
    print("\n💡 Горячие клавиши:")
    print("   Ctrl+F: Полный экран")
    print("   Ctrl+S: Скриншот")
    print("   Ctrl+O: Выкл/вкл экран")
    print("   Ctrl+R: Повернуть")
    print()
    
    time.sleep(1)
    
    result = run_cmd(cmd)
    
    if result.returncode != 0 and keyboard_mode == "1":
        print("\n⚠️  UHID не поддерживается!")
        print("Возможные причины:")
        print("- Android версия ниже 9")
        print("- Нет прав для UHID устройств")
        
        retry = input("\nПопробовать SDK режим? (y/n): ").strip().lower()
        if retry == 'y':
            cmd = [str(scrcpy_path), "--video-bit-rate", bitrate, "--max-size", maxsize, "--keyboard=sdk"]
            if connection_mode == "usb":
                cmd.extend(["--select-usb"])
            elif connection_mode == "wifi":
                cmd.extend(["--select-tcpip"])
            run_cmd(cmd)
    
    print("\n✅ Scrcpy завершен")


def main():
    try:
        console.print(Panel.fit(
            "[bold cyan]MK DroidScreenCast[/bold cyan]\n"
            "[dim]ADB + Scrcpy Manager[/dim]",
            border_style="cyan"
        ))
        
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console
        ) as progress:
            progress.add_task("Проверяю инструменты...", total=None)
            adb_path, scrcpy_path = ensure_tools()
        
        console.print(f"[green]✅ ADB:[/green] [dim]{adb_path}[/dim]")
        console.print(f"[green]✅ scrcpy:[/green] [dim]{scrcpy_path}[/dim]")
        
        os.environ["PATH"] = (
            str(adb_path.parent) + os.pathsep + 
            str(scrcpy_path.parent) + os.pathsep + 
            os.environ["PATH"]
        )
        
        print("\n🔄 Перезапускаю ADB...")
        run_cmd([str(adb_path), "kill-server"], show_output=False)
        time.sleep(1)
        run_cmd([str(adb_path), "start-server"], show_output=False)
        
        table = Table(box=box.ROUNDED, show_header=False, title="🔌 ВЫБЕРИТЕ МЕТОД", title_style="bold cyan")
        table.add_column("№", style="cyan", width=4)
        table.add_column("Описание", style="white")
        
        table.add_row("0", "⚡ Быстрое подключение (к сохраненному устройству)")
        table.add_row("1", "🔌 USB (прямое)")
        table.add_row("2", "📡 Беспроводная отладка (Android 11+)")
        table.add_row("3", "🔄 USB → Wi-Fi (любой Android)")
        
        console.print(table)
        
        choice = Prompt.ask("\n[cyan]Выберите метод[/cyan]", choices=["0", "1", "2", "3"], default="0")
        
        connected = False
        connection_mode = None
        
        if choice == "0":
            result = quick_connect(adb_path)
            if result is None:
                return main()
            connected = result
            connection_mode = "wifi"
        elif choice == "1":
            connected = usb_direct(adb_path)
            connection_mode = "usb"
        elif choice == "2":
            connected = wireless_pairing(adb_path)
            connection_mode = "wifi"
        elif choice == "3":
            connected = usb_to_wireless(adb_path)
            connection_mode = "wifi"
        
        if not connected:
            console.print("\n[red]❌ Не удалось подключиться[/red]")
            if Confirm.ask("\n[yellow]Вернуться в главное меню?[/yellow]", default=True):
                console.clear()
                return main()
            else:
                console.print("\n[dim]До свидания! 👋[/dim]")
                return
        
        launch_scrcpy(scrcpy_path, connection_mode)
        
    except KeyboardInterrupt:
        print("\n\n⏹️  Прервано")
    except Exception as e:
        print(f"\n❌ ОШИБКА: {e}")
        import traceback
        traceback.print_exc()

def show_startup_menu():
    console.print(Panel.fit(
        "[bold cyan]MK DroidScreenCast[/bold cyan]\n"
        "[dim]Выберите режим работы[/dim]",
        border_style="cyan"
    ))
    
    table = Table(box=box.ROUNDED, show_header=False)
    table.add_column("№", style="cyan", width=4)
    table.add_column("Описание", style="white")
    
    table.add_row("1", "🖥️  CLI режим (терминал)")
    table.add_row("2", "🌐 Web Panel (браузер)")
    
    console.print(table)
    
    choice = Prompt.ask("\n[cyan]Выберите режим[/cyan]", choices=["1", "2"], default="1")
    
    if choice == "2":
        from web_panel import run_server
        console.print("\n[green]🚀 Запускаю Web Panel...[/green]")
        console.print("[yellow]📱 Откройте браузер: http://localhost:6969[/yellow]\n")
        run_server()
    else:
        main()


if __name__ == "__main__":
    show_startup_menu()
