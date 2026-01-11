from datetime import datetime

from .config import load_config, save_config


def list_devices():
    return load_config().get("devices", [])


def save_device(name, ip, port, connection_type):
    config = load_config()
    devices = config.setdefault("devices", [])

    for device in devices:
        if device.get("ip") == ip and str(device.get("port")) == str(port):
            device["name"] = name
            device["connection_type"] = connection_type
            device["last_used"] = datetime.now().isoformat()
            break
    else:
        devices.append({
            "name": name,
            "ip": ip,
            "port": str(port),
            "connection_type": connection_type,
            "last_used": datetime.now().isoformat(),
        })

    save_config(config)


def remove_device(ip, port):
    config = load_config()
    port = str(port)
    config["devices"] = [
        device
        for device in config.get("devices", [])
        if not (device.get("ip") == ip and str(device.get("port")) == port)
    ]
    save_config(config)
