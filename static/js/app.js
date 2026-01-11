let ws;
const state = {
    lang: 'en',
    i18n: {},
    presets: [],
    languages: [],
    activeDevices: [],
    savedDevices: [],
    theme: 'light'
};
const FIRST_RUN_KEY = 'mkdsc_first_run_seen';

function t(key) {
    return state.i18n[key] || key;
}

function formatMessage(key, params = {}) {
    let text = t(key);
    Object.keys(params).forEach((param) => {
        text = text.replace(`{${param}}`, params[param]);
    });
    return text;
}

function openFirstRunModal() {
    const modal = document.getElementById('firstRunModal');
    if (!modal) return;
    modal.classList.add('show');
    modal.setAttribute('aria-hidden', 'false');
}

function closeFirstRunModal(markSeen = true) {
    const modal = document.getElementById('firstRunModal');
    if (!modal) return;
    const wasOpen = modal.classList.contains('show');
    modal.classList.remove('show');
    modal.setAttribute('aria-hidden', 'true');
    if (markSeen && wasOpen) {
        localStorage.setItem(FIRST_RUN_KEY, 'true');
    }
}

function maybeShowFirstRunModal() {
    const modal = document.getElementById('firstRunModal');
    if (!modal) return;
    if (localStorage.getItem(FIRST_RUN_KEY) === 'true') return;
    openFirstRunModal();
}

function parseDeviceAddress(serial) {
    if (!serial || !serial.includes(':')) return null;
    const [ip, port] = serial.split(':');
    if (!ip) return null;
    return { ip, port: port || '5555' };
}

async function saveDevice(name, ip, port) {
    const response = await fetch('/api/devices/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, ip, port })
    });
    return response.json();
}

async function saveDeviceFromForm() {
    const name = document.getElementById('saveDeviceName').value.trim();
    const ip = document.getElementById('saveDeviceIp').value.trim();
    const port = document.getElementById('saveDevicePort').value.trim() || '5555';

    if (!name) {
        showNotification('error', 'notification_name_required');
        return;
    }

    if (!ip) {
        showNotification('error', 'notification_address_required');
        return;
    }

    try {
        const data = await saveDevice(name, ip, port);
        if (data.success) {
            showNotification('success', 'notification_device_saved');
            document.getElementById('saveDeviceName').value = '';
            document.getElementById('saveDeviceIp').value = '';
            document.getElementById('saveDevicePort').value = '5555';
            loadDevices();
        } else {
            showNotification('error', 'notification_device_save_failed');
        }
    } catch (error) {
        showNotification('error', 'notification_device_save_failed');
    }
}

function fillSaveDeviceForm(serial) {
    const parsed = parseDeviceAddress(serial);
    if (!parsed) {
        showNotification('error', 'notification_address_required');
        return;
    }

    const ipInput = document.getElementById('saveDeviceIp');
    const portInput = document.getElementById('saveDevicePort');
    if (ipInput) ipInput.value = parsed.ip;
    if (portInput) portInput.value = parsed.port || '5555';

    const nameInput = document.getElementById('saveDeviceName');
    if (nameInput) {
        nameInput.focus();
    }
}

function renderScrcpyDevices(devices = state.activeDevices) {
    const select = document.getElementById('scrcpyDevice');
    if (!select) return;

    const current = select.value;
    select.innerHTML = '';

    const autoOption = document.createElement('option');
    autoOption.value = '';
    autoOption.textContent = t('scrcpy_device_auto');
    select.appendChild(autoOption);

    devices.forEach((device) => {
        const option = document.createElement('option');
        option.value = device.serial;
        option.textContent = `${device.serial} (${device.status})`;
        select.appendChild(option);
    });

    if (devices.some((device) => device.serial === current)) {
        select.value = current;
    } else {
        select.value = '';
    }
}

async function loadConfig() {
    const response = await fetch('/api/config');
    const data = await response.json();
    state.lang = data.language || 'en';
    state.presets = data.presets || [];
    state.languages = data.languages || ['en'];
}

async function loadI18n(lang) {
    const response = await fetch(`/api/i18n?lang=${encodeURIComponent(lang)}`);
    const data = await response.json();
    state.i18n = data.strings || {};
    document.documentElement.lang = lang;
    applyI18n();
}

function applyI18n() {
    document.querySelectorAll('[data-i18n]').forEach((element) => {
        const key = element.dataset.i18n;
        element.textContent = t(key);
    });

    document.querySelectorAll('[data-i18n-placeholder]').forEach((element) => {
        const key = element.dataset.i18nPlaceholder;
        element.placeholder = t(key);
    });

    updateStatus(ws && ws.readyState === WebSocket.OPEN);
    updateThemeToggle();
    renderPresets();
    renderScrcpyDevices();

    lucide.createIcons();
}

function initTheme() {
    const storedTheme = localStorage.getItem('theme');
    const normalizedTheme = storedTheme === 'dark' || storedTheme === 'light' ? storedTheme : null;
    const prefersDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    state.theme = normalizedTheme || (prefersDark ? 'dark' : 'light');
    document.documentElement.dataset.theme = state.theme;
}

function updateThemeToggle() {
    const button = document.getElementById('themeToggle');
    if (!button) return;

    const label = state.theme === 'dark' ? t('theme_dark') : t('theme_light');
    const icon = state.theme === 'dark' ? 'moon' : 'sun';

    button.setAttribute('aria-pressed', state.theme === 'dark');
    const labelEl = document.getElementById('themeToggleLabel');
    if (labelEl) labelEl.textContent = label;

    const iconEl = button.querySelector('[data-lucide]');
    if (iconEl) iconEl.setAttribute('data-lucide', icon);
    lucide.createIcons();
}

function toggleTheme() {
    state.theme = state.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = state.theme;
    localStorage.setItem('theme', state.theme);
    updateThemeToggle();
}

function renderLanguageOptions() {
    const select = document.getElementById('langSelect');
    select.innerHTML = '';

    state.languages.forEach((lang) => {
        const option = document.createElement('option');
        option.value = lang;
        option.textContent = lang.toUpperCase();
        select.appendChild(option);
    });

    select.value = state.lang;
}

function updateStatus(connected) {
    const badge = document.getElementById('wsStatus');
    badge.classList.toggle('offline', !connected);

    const label = connected ? t('status_online') : t('status_offline');
    badge.innerHTML = `<span class="status-dot"></span><span>${label}</span>`;
}

function connectWebSocket() {
    ws = new WebSocket(`ws://${window.location.host}/ws`);

    ws.onopen = () => updateStatus(true);
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

async function loadDevices() {
    try {
        const response = await fetch('/api/devices');
        const data = await response.json();
        updateActiveDevices(data.connected || []);
        updateSavedDevices(data.saved || []);
    } catch (error) {
        console.error('loadDevices error', error);
    }
}

function updateActiveDevices(devices) {
    if (!hasDeviceChanges(state.activeDevices, devices, ['serial', 'status'])) {
        return;
    }
    state.activeDevices = devices;
    renderScrcpyDevices(devices);

    const container = document.getElementById('activeDevices');

    if (!devices.length) {
        container.innerHTML = `
            <div class="empty-state">
                <i data-lucide="smartphone-off" style="width: 48px; height: 48px; opacity: 0.3;"></i>
                <p>${t('active_empty')}</p>
            </div>
        `;
        lucide.createIcons();
        return;
    }

    container.innerHTML = devices.map((device) => {
        const isWifi = device.serial.includes(':');
        const saveButton = isWifi
            ? `
                <button class="btn btn-secondary btn-sm" onclick="fillSaveDeviceForm('${device.serial}')">
                    <i data-lucide="save"></i>
                    ${t('device_save')}
                </button>
            `
            : '';

        return `
            <div class="device-item">
                <div class="device-info">
                    <div class="device-name">
                        <i data-lucide="${isWifi ? 'wifi' : 'usb'}" style="width: 16px; height: 16px;"></i>
                        ${device.serial}
                    </div>
                    <div class="device-address">${device.status}</div>
                </div>
                <div class="device-actions">
                    ${saveButton}
                    <button class="btn btn-danger btn-sm" onclick="disconnectDevice('${device.serial}')">
                        <i data-lucide="power-off"></i>
                        ${t('device_disconnect')}
                    </button>
                </div>
            </div>
        `;
    }).join('');

    lucide.createIcons();
}

function updateSavedDevices(devices) {
    if (!hasDeviceChanges(state.savedDevices, devices, ['ip', 'port', 'name', 'connection_type'])) {
        return;
    }
    state.savedDevices = devices;

    const container = document.getElementById('savedDevices');

    if (!devices.length) {
        container.innerHTML = `
            <div class="empty-state">
                <i data-lucide="save-off" style="width: 48px; height: 48px; opacity: 0.3;"></i>
                <p>${t('saved_empty')}</p>
            </div>
        `;
        lucide.createIcons();
        return;
    }

    container.innerHTML = devices.map((device) => `
        <div class="device-item">
            <div class="device-info">
                <div class="device-name">
                    <i data-lucide="${device.connection_type === 'wifi' ? 'wifi' : 'usb'}" style="width: 16px; height: 16px;"></i>
                    ${device.name}
                </div>
                <div class="device-address">${device.ip}:${device.port}</div>
            </div>
            <div class="device-actions">
                <button class="btn btn-primary btn-sm" onclick="connectSaved('${device.ip}:${device.port}')">
                    <i data-lucide="zap"></i>
                    ${t('device_connect')}
                </button>
                <button class="btn btn-danger btn-sm" onclick="deleteSaved('${device.ip}', '${device.port}')">
                    <i data-lucide="trash-2"></i>
                    ${t('device_delete')}
                </button>
            </div>
        </div>
    `).join('');

    lucide.createIcons();
}

function hasDeviceChanges(previous, next, keys) {
    if (previous.length !== next.length) return true;
    for (let index = 0; index < next.length; index += 1) {
        const prevItem = previous[index] || {};
        const nextItem = next[index] || {};
        for (const key of keys) {
            if (prevItem[key] !== nextItem[key]) {
                return true;
            }
        }
    }
    return false;
}

async function quickConnect() {
    const address = document.getElementById('quickConnectAddress').value.trim();
    if (!address) {
        showNotification('error', 'notification_address_required');
        return;
    }

    try {
        const response = await fetch('/api/connect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });

        const data = await response.json();
        showNotification(data.success ? 'success' : 'error', data.success ? 'notification_connected' : 'notification_connect_failed');

        if (data.success) {
            document.getElementById('quickConnectAddress').value = '';
            loadDevices();
        }
    } catch (error) {
        showNotification('error', 'notification_connect_failed');
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
        showNotification(data.success ? 'success' : 'error', data.success ? 'notification_connected' : 'notification_connect_failed');
        loadDevices();
    } catch (error) {
        showNotification('error', 'notification_connect_failed');
    }
}

async function disconnectDevice(address) {
    try {
        await fetch('/api/disconnect', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ address })
        });

        showNotification('success', 'notification_disconnected');
        loadDevices();
    } catch (error) {
        showNotification('error', 'notification_connect_failed');
    }
}

async function deleteSaved(ip, port) {
    if (!confirm(t('device_delete') + '?')) return;

    try {
        await fetch(`/api/devices/${ip}/${port}`, { method: 'DELETE' });
        showNotification('success', 'notification_saved_deleted');
        loadDevices();
    } catch (error) {
        showNotification('error', 'notification_connect_failed');
    }
}

async function pairDevice() {
    const pair_address = document.getElementById('pairAddress').value.trim();
    const pair_code = document.getElementById('pairCode').value.trim();

    if (!pair_address || !pair_code) {
        showNotification('error', 'notification_address_required');
        return;
    }

    try {
        const response = await fetch('/api/pair', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ pair_address, pair_code })
        });

        const data = await response.json();
        showNotification(data.success ? 'success' : 'error', data.success ? 'notification_pair_success' : 'notification_pair_failed');

        if (data.success) {
            document.getElementById('pairAddress').value = '';
            document.getElementById('pairCode').value = '';
        }
    } catch (error) {
        showNotification('error', 'notification_pair_failed');
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
            showNotification('success', 'notification_tcpip_success');
            document.getElementById('quickConnectAddress').value = `${data.ip}:${port}`;
        } else {
            showNotification('error', 'notification_tcpip_failed');
        }
    } catch (error) {
        showNotification('error', 'notification_tcpip_failed');
    }
}

function renderPresets() {
    const presetSelect = document.getElementById('scrcpyPreset');
    const presetList = document.getElementById('presetList');

    presetSelect.innerHTML = '';
    const customOption = document.createElement('option');
    customOption.value = 'custom';
    customOption.textContent = t('scrcpy_preset_custom');
    presetSelect.appendChild(customOption);

    state.presets.forEach((preset) => {
        const option = document.createElement('option');
        option.value = preset.name;
        option.textContent = `${preset.name} (${preset.bitrate}/${preset.maxsize}px)`;
        presetSelect.appendChild(option);
    });

    presetSelect.value = 'custom';

    presetList.innerHTML = '';
    if (!state.presets.length) {
        presetList.innerHTML = `<p class="muted">${t('preset_empty')}</p>`;
        return;
    }

    state.presets.forEach((preset) => {
        const item = document.createElement('div');
        item.className = 'preset-item';
        item.innerHTML = `
            <div class="preset-meta">
                <strong>${preset.name}</strong>
                <span>${preset.bitrate} / ${preset.maxsize}px</span>
            </div>
            <div class="device-actions">
                <button class="btn btn-primary btn-sm" data-action="apply">
                    ${t('preset_apply_button')}
                </button>
                <button class="btn btn-danger btn-sm" data-action="delete">
                    ${t('preset_delete_button')}
                </button>
            </div>
        `;

        item.querySelector('[data-action="apply"]').addEventListener('click', () => {
            applyPreset(preset.name);
        });
        item.querySelector('[data-action="delete"]').addEventListener('click', () => {
            removePreset(preset.name);
        });

        presetList.appendChild(item);
    });
}

function applyPreset(name) {
    const preset = state.presets.find((item) => item.name === name);
    if (!preset) return;

    document.getElementById('bitrateNum').value = parseInt(preset.bitrate, 10) || 8;
    document.getElementById('maxsize').value = preset.maxsize;
    document.getElementById('scrcpyPreset').value = preset.name;
}

async function savePreset() {
    const name = document.getElementById('presetName').value.trim();
    if (!name) {
        showNotification('error', 'notification_name_required');
        return;
    }

    const bitrate = `${document.getElementById('bitrateNum').value.trim()}M`;
    const maxsize = document.getElementById('maxsize').value.trim();

    try {
        const response = await fetch('/api/presets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, bitrate, maxsize })
        });
        const data = await response.json();
        state.presets = data.presets || [];
        document.getElementById('presetName').value = '';
        renderPresets();
        showNotification('success', 'notification_preset_saved');
    } catch (error) {
        showNotification('error', 'notification_preset_failed');
    }
}

async function removePreset(name) {
    try {
        const response = await fetch(`/api/presets/${encodeURIComponent(name)}`, { method: 'DELETE' });
        const data = await response.json();
        state.presets = data.presets || [];
        renderPresets();
        showNotification('success', 'notification_preset_deleted');
    } catch (error) {
        showNotification('error', 'notification_preset_failed');
    }
}

async function launchScrcpy() {
    const bitrateValue = document.getElementById('bitrateNum').value.trim();
    const serial = document.getElementById('scrcpyDevice').value;
    const params = {
        bitrate: `${bitrateValue}M`,
        maxsize: document.getElementById('maxsize').value.trim(),
        keyboard: document.getElementById('keyboard').value,
        connection: document.getElementById('connection').value,
        preset: document.getElementById('scrcpyPreset').value,
        stay_awake: document.getElementById('stayAwake').checked,
        show_touches: document.getElementById('showTouches').checked,
        turn_screen_off: document.getElementById('turnScreenOff').checked,
        fullscreen: document.getElementById('fullscreen').checked,
        no_audio: document.getElementById('noAudio').checked
    };
    if (serial) {
        params.serial = serial;
    }

    try {
        const response = await fetch('/api/scrcpy/launch', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(params)
        });
        const data = await response.json();

        if (data.success) {
            showNotification('success', 'notification_scrcpy_started');
            if (data.warning_key) {
                showNotification('error', data.warning_key);
            }
        } else {
            showNotification('error', 'notification_scrcpy_failed');
        }
    } catch (error) {
        showNotification('error', 'notification_scrcpy_failed');
    }
}

function downloadLogs() {
    showNotification('success', 'notification_download_started');
    window.location.href = '/api/logs/download';
}

function showNotification(type, key) {
    const message = t(key);
    showNotificationMessage(type, message);
}

function showNotificationMessage(type, message) {
    const notif = document.createElement('div');
    notif.className = `notification notification-${type}`;

    const icon = type === 'success' ? 'check-circle' : 'alert-circle';
    notif.innerHTML = `
        <i data-lucide="${icon}" style="width: 20px; height: 20px;"></i>
        <span>${message}</span>
    `;

    document.body.appendChild(notif);
    lucide.createIcons();

    setTimeout(() => notif.remove(), 3000);
}

async function checkUpdates() {
    try {
        const response = await fetch('/api/update/check');
        const data = await response.json();

        if (data.update_available) {
            const body = (data.release && data.release.body || '').trim();
            const promptText = `${formatMessage('update_available_prompt', { latest: data.latest || '' })}` +
                (body ? `\n\n${body}` : '');

            if (confirm(promptText)) {
                const applyResponse = await fetch('/api/update/apply', { method: 'POST' });
                const result = await applyResponse.json();
                if (result.success) {
                    showNotificationMessage('success', result.message || t('notification_update_applied'));
                } else {
                    showNotificationMessage('error', result.message || t('notification_update_failed'));
                }
            }
        } else {
            showNotification('success', 'notification_update_latest');
        }
    } catch (error) {
        showNotification('error', 'notification_update_failed');
    }
}

async function init() {
    initTheme();
    await loadConfig();
    await loadI18n(state.lang);
    renderLanguageOptions();
    renderPresets();
    connectWebSocket();
    loadDevices();

    document.getElementById('langSelect').addEventListener('change', async (event) => {
        state.lang = event.target.value;
        await fetch('/api/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ language: state.lang })
        });
        await loadI18n(state.lang);
    });

    document.getElementById('quickConnectBtn').addEventListener('click', quickConnect);
    document.getElementById('pairBtn').addEventListener('click', pairDevice);
    document.getElementById('tcpipBtn').addEventListener('click', enableTcpip);
    document.getElementById('launchScrcpyBtn').addEventListener('click', launchScrcpy);
    document.getElementById('savePresetBtn').addEventListener('click', savePreset);
    const saveDeviceBtn = document.getElementById('saveDeviceBtn');
    if (saveDeviceBtn) {
        saveDeviceBtn.addEventListener('click', saveDeviceFromForm);
    }
    document.getElementById('downloadLogsBtn').addEventListener('click', downloadLogs);
    document.getElementById('themeToggle').addEventListener('click', toggleTheme);
    const checkUpdatesBtn = document.getElementById('checkUpdatesBtn');
    if (checkUpdatesBtn) {
        checkUpdatesBtn.addEventListener('click', checkUpdates);
    }

    document.getElementById('scrcpyPreset').addEventListener('change', (event) => {
        if (event.target.value !== 'custom') {
            applyPreset(event.target.value);
        }
    });

    document.getElementById('quickConnectAddress').addEventListener('keypress', (event) => {
        if (event.key === 'Enter') {
            quickConnect();
        }
    });

    document.getElementById('bitrateNum').addEventListener('input', () => {
        document.getElementById('scrcpyPreset').value = 'custom';
    });

    document.getElementById('maxsize').addEventListener('input', () => {
        document.getElementById('scrcpyPreset').value = 'custom';
    });

    const modal = document.getElementById('firstRunModal');
    const modalClose = document.getElementById('firstRunClose');
    const modalDone = document.getElementById('firstRunDone');
    if (modalClose) modalClose.addEventListener('click', () => closeFirstRunModal());
    if (modalDone) modalDone.addEventListener('click', () => closeFirstRunModal());
    if (modal) {
        modal.addEventListener('click', (event) => {
            if (event.target === modal) {
                closeFirstRunModal();
            }
        });
    }
    document.addEventListener('keydown', (event) => {
        if (event.key === 'Escape') {
            closeFirstRunModal();
        }
    });

    maybeShowFirstRunModal();
}

init();
setInterval(loadDevices, 5000);
