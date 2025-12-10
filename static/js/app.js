let ws;

function connectWebSocket() {
    ws = new WebSocket(`ws://${window.location.host}/ws`);
    
    ws.onopen = () => {
        updateStatus(true);
    };
    
    ws.onclose = () => {
        updateStatus(false);
        setTimeout(connectWebSocket, 3000);
    };
    
    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        if (data.type === 'devices_update') {
            updateActiveDevices(data.devices);
        }
    };
}

function updateStatus(connected) {
    const badge = document.getElementById('wsStatus');
    if (connected) {
        badge.classList.remove('offline');
        badge.innerHTML = '<span class="status-dot"></span>WebSocket';
    } else {
        badge.classList.add('offline');
        badge.innerHTML = '<span class="status-dot"></span>Отключено';
    }
}

async function loadDevices() {
    try {
        const response = await fetch('/api/devices');
        const data = await response.json();
        updateActiveDevices(data.connected);
        updateSavedDevices(data.saved);
    } catch (error) {
        console.error('Ошибка:', error);
    }
}

function updateActiveDevices(devices) {
    const container = document.getElementById('activeDevices');
    
    if (devices.length === 0) {
        container.innerHTML = `
            <div class="empty-state">
                <i data-lucide="smartphone-off" style="width: 48px; height: 48px; opacity: 0.3;"></i>
                <p>Нет подключенных устройств</p>
            </div>
        `;
        lucide.createIcons();
        return;
    }
    
    container.innerHTML = devices.map(device => `
        <div class="device-item">
            <div class="device-info">
                <div class="device-name">
                    <i data-lucide="${device.serial.includes(':') ? 'wifi' : 'usb'}" style="width: 16px; height: 16px;"></i>
                    ${device.serial}
                </div>
                <div class="device-address">${device.status}</div>
            </div>
            <button class="btn btn-danger btn-sm" onclick="disconnectDevice('${device.serial}')">
                <i data-lucide="power-off"></i>
                Отключить
            </button>
        </div>
    `).join('');
    
    lucide.createIcons();
}

function updateSavedDevices(devices) {
    const container = document.getElementById('savedDevices');
    
    if (devices.length === 0) {
        container.innerHTML = `
            <div class="empty-state">
                <i data-lucide="save-off" style="width: 48px; height: 48px; opacity: 0.3;"></i>
                <p>Нет сохраненных устройств</p>
            </div>
        `;
        lucide.createIcons();
        return;
    }
    
    container.innerHTML = devices.map(device => `
        <div class="device-item">
            <div class="device-info">
                <div class="device-name">
                    <i data-lucide="${device.connection_type === 'wifi' ? 'wifi' : 'usb'}" style="width: 16px; height: 16px;"></i>
                    ${device.name}
                </div>
                <div class="device-address">${device.ip}:${device.port}</div>
            </div>
            <div style="display: flex; gap: 5px;">
                <button class="btn btn-primary btn-sm" onclick="connectSaved('${device.ip}:${device.port}')">
                    <i data-lucide="zap"></i>
                    Подключить
                </button>
                <button class="btn btn-danger btn-sm" onclick="deleteSaved('${device.ip}', '${device.port}')">
                    <i data-lucide="trash-2"></i>
                </button>
            </div>
        </div>
    `).join('');
    
    lucide.createIcons();
}

async function quickConnect() {
    const address = document.getElementById('quickConnectAddress').value.trim();
    if (!address) {
        showNotification('error', 'Введите адрес!');
        return;
    }
    
    try {
        const response = await fetch('/api/connect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });
        
        const data = await response.json();
        showNotification(
            data.success ? 'success' : 'error',
            data.success ? 'Подключено!' : 'Ошибка подключения'
        );
        
        if (data.success) {
            document.getElementById('quickConnectAddress').value = '';
            loadDevices();
        }
    } catch (error) {
        showNotification('error', 'Ошибка сети');
    }
}

async function connectSaved(address) {
    try {
        const response = await fetch('/api/connect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });
        
        const data = await response.json();
        showNotification(
            data.success ? 'success' : 'error',
            data.success ? `Подключено к ${address}` : 'Ошибка'
        );
        
        loadDevices();
    } catch (error) {
        showNotification('error', 'Ошибка сети');
    }
}

async function disconnectDevice(address) {
    try {
        await fetch('/api/disconnect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });
        
        showNotification('success', 'Отключено');
        loadDevices();
    } catch (error) {
        showNotification('error', 'Ошибка');
    }
}

async function deleteSaved(ip, port) {
    if (!confirm('Удалить это устройство?')) return;
    
    try {
        await fetch(`/api/devices/${ip}/${port}`, { method: 'DELETE' });
        showNotification('success', 'Устройство удалено');
        loadDevices();
    } catch (error) {
        showNotification('error', 'Ошибка');
    }
}

async function pairDevice() {
    const pair_address = document.getElementById('pairAddress').value.trim();
    const pair_code = document.getElementById('pairCode').value.trim();
    
    if (!pair_address || !pair_code) {
        showNotification('error', 'Заполните все поля!');
        return;
    }
    
    try {
        const response = await fetch('/api/pair', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ pair_address, pair_code })
        });
        
        const data = await response.json();
        showNotification(
            data.success ? 'success' : 'error',
            data.success ? 'Сопряжение успешно!' : 'Ошибка сопряжения'
        );
        
        if (data.success) {
            document.getElementById('pairAddress').value = '';
            document.getElementById('pairCode').value = '';
        }
    } catch (error) {
        showNotification('error', 'Ошибка сети');
    }
}

async function enableTcpip() {
    const port = document.getElementById('tcpipPort').value.trim();
    
    try {
        const response = await fetch('/api/tcpip', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ port })
        });
        
        const data = await response.json();
        
        if (data.success && data.ip) {
            showNotification('success', `Режим tcpip включен! IP: ${data.ip}:${port}`);
            document.getElementById('quickConnectAddress').value = `${data.ip}:${port}`;
        } else {
            showNotification('error', 'Ошибка');
        }
    } catch (error) {
        showNotification('error', 'Ошибка сети');
    }
}

function applyPreset() {
    const preset = document.getElementById('scrcpyPreset').value;
    const bitrateInput = document.getElementById('bitrateNum');
    const maxsizeInput = document.getElementById('maxsize');

    if (preset === '1080p') {
        bitrateInput.value = 8;
        maxsizeInput.value = 1080;
    } else if (preset === '2k') {
        bitrateInput.value = 16;
        maxsizeInput.value = 1440;
    } else if (preset === '4k') {
        bitrateInput.value = 32;
        maxsizeInput.value = 2160;
    }
    syncBitrate('num');
}

function syncBitrate(source) {
    const numInput = document.getElementById('bitrateNum');
    const hiddenInput = document.getElementById('bitrate');
    
    hiddenInput.value = numInput.value + 'M';
    
    if (document.activeElement === numInput) {
        document.getElementById('scrcpyPreset').value = 'custom';
    }
}

async function launchScrcpy() {
    const params = {
        bitrate: document.getElementById('bitrate').value.trim(), 
        maxsize: document.getElementById('maxsize').value.trim(),
        keyboard: document.getElementById('keyboard').value,
        connection: document.getElementById('connection').value,
        stayawake: document.getElementById('stayAwake').checked,
        showtouches: document.getElementById('showTouches').checked,
        fullscreen: document.getElementById('fullscreen').checked,
        noaudio: document.getElementById('noAudio')?.checked || false
    };

    try {
        const response = await fetch('/api/scrcpy/launch', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(params)
        });
        const data = await response.json();
        
        if (data.success) {
            showNotification('success', 'scrcpy запущен!');
        } else {
            showNotification('error', 'Ошибка запуска: ' + JSON.stringify(data));
        }
    } catch (error) {
        showNotification('error', 'Ошибка сети: ' + error);
    }
}

function showNotification(type, message) {
    const notif = document.createElement('div');
    notif.className = `notification notification-${type}`;
    
    const icon = type === 'success' ? 'check-circle' : 'alert-circle';
    notif.innerHTML = `
        <i data-lucide="${icon}" style="width: 20px; height: 20px;"></i>
        ${message}
    `;
    
    document.body.appendChild(notif);
    lucide.createIcons();
    
    setTimeout(() => notif.remove(), 3000);
}

connectWebSocket();
loadDevices();
setInterval(loadDevices, 5000);

document.addEventListener('DOMContentLoaded', () => {
    document.getElementById('quickConnectAddress')?.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') quickConnect();
    });
});
