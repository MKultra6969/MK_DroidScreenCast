import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BootOverlay } from './components/overlays/BootOverlay';
import { FirstRunModal } from './components/overlays/FirstRunModal';
import { Notifications } from './components/overlays/Notifications';
import { RecordingBanner } from './components/overlays/RecordingBanner';
import { Footer } from './components/layout/Footer';
import { Header } from './components/layout/Header';
import { Sidebar } from './components/layout/Sidebar';
import { AutomationPage } from './features/automation/AutomationPage';
import { FilesPage } from './features/files/FilesPage';
import { HomePage } from './features/home/HomePage';
import { ServiceMenuPage } from './features/service/ServiceMenuPage';
import { apiFetch, readJson } from './lib/api';
import { confirmAction } from './lib/dialogs';
import { DEVICE_STREAM_TIMEOUT_MS, listenDevicesUpdate } from './lib/events';
import { getPageForSection } from './lib/navigation';
import {
  canSelfUpdate,
  checkForUpdate,
  installUpdate,
  openReleasePage,
  restartApp
} from './lib/updater';
import { FIRST_RUN_KEY } from './lib/storage';
import { formatDuration } from './lib/time';
import { initialFormState } from './state/form';
import type { Device, FileEntry, FormState, Notification, Preset, RecordingStatus, SavedDevice, Screenshot } from './types/app';
import { cn } from './utils';

const normalizeDevices = (value: unknown): Device[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      if (!item || typeof item !== 'object') return null;
      const record = item as { serial?: unknown; status?: unknown };
      const serial = typeof record.serial === 'string' ? record.serial.trim() : '';
      if (!serial) return null;
      const status = typeof record.status === 'string' ? record.status : 'unknown';
      return { serial, status };
    })
    .filter((item): item is Device => Boolean(item));
};

const normalizeSavedDevices = (value: unknown): SavedDevice[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item): SavedDevice | null => {
      if (!item || typeof item !== 'object') return null;
      const record = item as {
        ip?: unknown;
        port?: unknown;
        name?: unknown;
        connection_type?: unknown;
      };
      const ip = typeof record.ip === 'string' ? record.ip.trim() : '';
      const port =
        typeof record.port === 'string'
          ? record.port.trim()
          : typeof record.port === 'number'
            ? String(record.port)
            : '5555';
      const name = typeof record.name === 'string' ? record.name.trim() : '';
      if (!ip || !name) return null;
      const connection_type = typeof record.connection_type === 'string' ? record.connection_type : undefined;
      return { ip, port, name, connection_type };
    })
    .filter((item): item is SavedDevice => Boolean(item));
};

function App() {
  const [lang, setLang] = useState('en');
  const [i18n, setI18n] = useState<Record<string, string>>({});
  const [presets, setPresets] = useState<Preset[]>([]);
  const [languages, setLanguages] = useState<string[]>([]);
  const [appVersion, setAppVersion] = useState('');
  const [activeDevices, setActiveDevices] = useState<Device[]>([]);
  const [savedDevices, setSavedDevices] = useState<SavedDevice[]>([]);
  const [theme, setTheme] = useState<'light' | 'dark'>('light');
  const [themePreference, setThemePreference] = useState<'auto' | 'light' | 'dark'>('auto');
  const [wsConnected, setWsConnected] = useState(false);
  const [form, setForm] = useState<FormState>(initialFormState);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [recordingStatus, setRecordingStatus] = useState<RecordingStatus | null>(null);
  const [recordingLoading, setRecordingLoading] = useState(false);
  const [recordingSaving, setRecordingSaving] = useState(false);
  // A ticking timestamp rather than a counter, so the elapsed-time memo has a
  // dependency it genuinely reads.
  const [recordingNow, setRecordingNow] = useState(() => Date.now());
  const [firstRunOpen, setFirstRunOpen] = useState(false);
  const [devicesLoading, setDevicesLoading] = useState(true);
  const [savedLoading, setSavedLoading] = useState(true);
  const [logsExportDir, setLogsExportDir] = useState('');
  const [downloadsBaseDir, setDownloadsBaseDir] = useState('');
  const [configDraft, setConfigDraft] = useState('');
  const [configLoading, setConfigLoading] = useState(false);
  const [configSaving, setConfigSaving] = useState(false);
  const [autoConnectionEnabled, setAutoConnectionEnabled] = useState(false);
  const [autoConnectionSaving, setAutoConnectionSaving] = useState(false);
  const [appReady, setAppReady] = useState(false);
  const [bootTimedOut, setBootTimedOut] = useState(false);
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({});
  const [activeSection, setActiveSection] = useState('faq');
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [sidebarMobileOpen, setSidebarMobileOpen] = useState(false);
  const [expandedMenuGroups, setExpandedMenuGroups] = useState<Record<string, boolean>>({
    devices: true,
    tools: true,
    files: false,
    automation: false,
    settings: false
  });

  // Service Menu state
  const [serviceOutput, setServiceOutput] = useState<string>('');
  const [serviceLoading, setServiceLoading] = useState(false);
  const [serviceCommand, setServiceCommand] = useState('');

  // Screenshots Gallery state
  const [screenshots, setScreenshots] = useState<Screenshot[]>([]);
  const [screenshotsLoading, setScreenshotsLoading] = useState(false);
  const [takingScreenshot, setTakingScreenshot] = useState(false);
  const [screenshotsPage, setScreenshotsPage] = useState(1);
  const [screenshotsPageSize, setScreenshotsPageSize] = useState(24);
  const [screenshotsTotal, setScreenshotsTotal] = useState(0);
  const [screenshotsTotalPages, setScreenshotsTotalPages] = useState(1);

  // File Manager state
  const [currentPath, setCurrentPath] = useState('/sdcard');
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [filesBusy, setFilesBusy] = useState(false);
  const [filesError, setFilesError] = useState('');
  const [filesPage, setFilesPage] = useState(1);
  const [filesPageSize, setFilesPageSize] = useState(100);
  const [filesTotal, setFilesTotal] = useState(0);
  const [filesTotalPages, setFilesTotalPages] = useState(1);

  const [bootProgress, setBootProgress] = useState('');
  const [updateProgress, setUpdateProgress] = useState('');
  const [serviceCommands, setServiceCommands] = useState<string[]>([]);

  const saveNameRef = useRef<HTMLInputElement>(null);
  const streamTimerRef = useRef<number | null>(null);
  const notificationIdRef = useRef(0);
  const lastRecordingErrorRef = useRef<string | null>(null);
  // Опрос бутстрапа переживает перезапуск эффекта; предупреждение о версии
  // scrcpy показываем один раз, а не на каждом тике.
  const scrcpyVersionWarnedRef = useRef(false);

  const t = useCallback((key: string) => i18n[key] || key, [i18n]);

  const formatMessage = useCallback(
    (key: string, params: Record<string, string> = {}) => {
      let text = t(key);
      Object.keys(params).forEach((param) => {
        text = text.replace(`{${param}}`, params[param]);
      });
      return text;
    },
    [t]
  );

  const notifyMessage = useCallback((type: Notification['type'], message: string) => {
    // Date.now() + random summed to the same value often enough to produce
    // duplicate React keys; a monotonic counter cannot collide.
    notificationIdRef.current += 1;
    const id = notificationIdRef.current;
    setNotifications((prev) => [...prev, { id, type, message }]);
    window.setTimeout(() => {
      setNotifications((prev) => prev.filter((item) => item.id !== id));
    }, 3000);
  }, []);

  const notify = useCallback(
    (type: Notification['type'], key: string) => {
      notifyMessage(type, t(key));
    },
    [notifyMessage, t]
  );

  const initTheme = useCallback(() => {
    const storedPref = localStorage.getItem('themePreference') as 'auto' | 'light' | 'dark' | null;
    const pref = storedPref || 'auto';
    setThemePreference(pref);

    const getSystemTheme = (): 'light' | 'dark' => {
      return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    };

    const applyTheme = (themePref: 'auto' | 'light' | 'dark') => {
      const nextTheme = themePref === 'auto' ? getSystemTheme() : themePref;
      setTheme(nextTheme);
      document.documentElement.dataset.theme = nextTheme;
    };

    applyTheme(pref);

    if (pref === 'auto' && window.matchMedia) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = () => {
        const storedPrefNow = localStorage.getItem('themePreference');
        if (storedPrefNow === 'auto' || !storedPrefNow) {
          applyTheme('auto');
        }
      };
      mediaQuery.addEventListener('change', handleChange);
    }
  }, []);

  const handleThemePreferenceChange = useCallback((newPref: 'auto' | 'light' | 'dark') => {
    setThemePreference(newPref);
    localStorage.setItem('themePreference', newPref);

    const getSystemTheme = (): 'light' | 'dark' => {
      return window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    };

    const nextTheme = newPref === 'auto' ? getSystemTheme() : newPref;
    setTheme(nextTheme);
    document.documentElement.dataset.theme = nextTheme;
  }, []);

  const toggleTheme = useCallback(() => {
    const nextTheme = theme === 'dark' ? 'light' : 'dark';
    handleThemePreferenceChange(nextTheme);
  }, [theme, handleThemePreferenceChange]);

  const loadConfig = useCallback(async () => {
    const response = await apiFetch('/api/config', { timeoutMs: 4000 });
    if (!response.ok) {
      throw new Error(`config request failed: ${response.status}`);
    }
    return response.json();
  }, []);

  const loadI18n = useCallback(async (nextLang: string) => {
    const response = await apiFetch(`/api/i18n?lang=${encodeURIComponent(nextLang)}`, { timeoutMs: 4000 });
    if (!response.ok) {
      throw new Error(`i18n request failed: ${response.status}`);
    }
    const data = await response.json();
    setI18n(data.strings || {});
    document.documentElement.lang = nextLang;
  }, []);

  const initializeApp = useCallback(async () => {
    setBootTimedOut(false);
    setAppReady(false);
    setDevicesLoading(true);
    setSavedLoading(true);

    const maxAttempts = 40;
    for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
      try {
        const data = await loadConfig();
        const nextLang = data.language || 'en';
        setLang(nextLang);
        setPresets(data.presets || []);
        setLanguages(data.languages || ['en']);
        setAppVersion(typeof data.version === 'string' ? data.version : '');
        setLogsExportDir(data.logs?.export_dir || '');
        setDownloadsBaseDir(data.downloads?.base_dir || '');
        setAutoConnectionEnabled(Boolean(data.connection_optimizer?.auto_switch));
        const recording = data.recording || {};
        setForm((prev) => ({
          ...prev,
          recordingFormat: recording.format || prev.recordingFormat,
          recordingAudioSource: recording.audio_source || prev.recordingAudioSource,
          recordingOutputDir: recording.output_dir || '',
          recordingFilePrefix: recording.file_prefix || prev.recordingFilePrefix,
          recordingShowPreview:
            recording.show_preview === undefined ? prev.recordingShowPreview : recording.show_preview,
          recordingStayAwake:
            recording.stay_awake === undefined ? prev.recordingStayAwake : recording.stay_awake,
          recordingShowTouches:
            recording.show_touches === undefined ? prev.recordingShowTouches : recording.show_touches,
          recordingTurnScreenOff:
            recording.turn_screen_off === undefined
              ? prev.recordingTurnScreenOff
              : recording.turn_screen_off
        }));
        await loadI18n(nextLang);
        setAppReady(true);
        return;
      } catch (error) {
        if (attempt < maxAttempts - 1) {
          await new Promise((resolve) =>
            window.setTimeout(resolve, Math.min(1400, 400 + attempt * 60))
          );
        }
      }
    }

    setBootTimedOut(true);
  }, [loadConfig, loadI18n]);

  const loadBootProgress = useCallback(async () => {
    try {
      const response = await apiFetch('/api/bootstrap/status', { timeoutMs: 4000 });
      if (!response.ok) return true;
      const data = await response.json();
      // Версия scrcpy вне проверенного диапазона не мешает запуску, но часть
      // опций может молча не сработать — предупреждаем.
      if (data.scrcpy_version_warning && !scrcpyVersionWarnedRef.current) {
        scrcpyVersionWarnedRef.current = true;
        notifyMessage('error', String(data.scrcpy_version_warning));
      }
      if (data.ready) {
        setBootProgress('');
        return true;
      }
      if (data.error) {
        setBootProgress(String(data.error));
        return true;
      }
      const tool = data.tool || 'tools';
      if (data.stage === 'downloading' && data.total_bytes > 0) {
        const percent = Math.round((data.downloaded_bytes / data.total_bytes) * 100);
        setBootProgress(formatMessage('boot_downloading', { tool, percent: String(percent) }));
      } else if (data.stage === 'verifying') {
        setBootProgress(formatMessage('boot_verifying', { tool }));
      } else if (data.stage === 'extracting') {
        setBootProgress(formatMessage('boot_extracting', { tool }));
      } else {
        setBootProgress(t('boot_preparing'));
      }
      return false;
    } catch {
      return true;
    }
  }, [formatMessage, notifyMessage, t]);

  const loadDevices = useCallback(async () => {
    try {
      const response = await apiFetch('/api/devices');
      if (!response.ok) {
        throw new Error(`devices request failed: ${response.status}`);
      }
      const data = await response.json();
      setActiveDevices(normalizeDevices(data.connected));
      setSavedDevices(normalizeSavedDevices(data.saved));
    } catch (error) {
      console.error('loadDevices error', error);
    } finally {
      setDevicesLoading(false);
      setSavedLoading(false);
    }
  }, []);

  const loadRecordingStatus = useCallback(async () => {
    try {
      const response = await apiFetch('/api/recording/status');
      if (!response.ok) return;
      const data = await response.json();
      const lastError = data?.last_error;
      if (!data?.active && lastError) {
        const errorKey = lastError.timestamp || lastError.output || `${lastError.exit_code || ''}`;
        if (errorKey && errorKey !== lastRecordingErrorRef.current) {
          const message = lastError.output || t('notification_recording_failed');
          notifyMessage('error', message);
          lastRecordingErrorRef.current = errorKey;
        }
      }
      setRecordingStatus(data);
    } catch (error) {
      console.error('loadRecordingStatus error', error);
    }
  }, [notifyMessage, t]);

  const clearStreamTimer = useCallback(() => {
    if (streamTimerRef.current) {
      window.clearTimeout(streamTimerRef.current);
      streamTimerRef.current = null;
    }
  }, []);

  /**
   * Отмечает, что поток устройств жив.
   *
   * `wsConnected` кормит двух потребителей: индикатор online/offline в боковой
   * панели и запасной HTTP-поллинг ниже. У событий Tauri нет соединения,
   * которое можно спросить, поэтому «онлайн» здесь — «событие приходило
   * недавно»: таймер перезапускается на каждом событии и гасит флаг, если
   * поток встал. Оставить флаг всегда `false` нельзя — вернулся бы двойной
   * опрос adb, всегда `true` — пропал бы запасной путь.
   */
  const markDeviceStreamAlive = useCallback(() => {
    setWsConnected(true);
    clearStreamTimer();
    streamTimerRef.current = window.setTimeout(() => {
      streamTimerRef.current = null;
      setWsConnected(false);
    }, DEVICE_STREAM_TIMEOUT_MS);
  }, [clearStreamTimer]);

  useEffect(() => {
    initTheme();
    void initializeApp();
  }, [initTheme, initializeApp]);

  // Поток устройств — события Tauri.
  useEffect(() => {
    if (!appReady) return;

    setDevicesLoading(true);
    setSavedLoading(true);
    void loadDevices();

    // Отписка приезжает промисом. Если эффект успел размонтироваться раньше,
    // снимаем её сразу: иначе после нескольких перезагрузок страницы
    // накопились бы живые обработчики и каждое событие обрабатывалось бы
    // столько раз, сколько было монтирований.
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void listenDevicesUpdate((payload) => {
      setActiveDevices(normalizeDevices(payload.devices));
      setDevicesLoading(false);
      markDeviceStreamAlive();
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch((error) => console.error('device stream error', error));

    return () => {
      cancelled = true;
      unlisten?.();
      clearStreamTimer();
      setWsConnected(false);
    };
  }, [appReady, clearStreamTimer, loadDevices, markDeviceStreamAlive]);

  // HTTP polling is only a fallback: while the device stream is up it already
  // pushes every change, and each poll costs another blocking adb call.
  useEffect(() => {
    if (!appReady || wsConnected) return;
    const intervalId = window.setInterval(loadDevices, 5000);
    return () => window.clearInterval(intervalId);
  }, [appReady, wsConnected, loadDevices]);

  useEffect(() => {
    if (!appReady) return;
    void loadRecordingStatus();
    const intervalId = window.setInterval(loadRecordingStatus, 2000);
    return () => window.clearInterval(intervalId);
  }, [appReady, loadRecordingStatus]);

  useEffect(() => {
    if (!recordingStatus?.active) return;
    setRecordingNow(Date.now());
    const intervalId = window.setInterval(() => setRecordingNow(Date.now()), 1000);
    return () => window.clearInterval(intervalId);
  }, [recordingStatus?.active]);

  useEffect(() => {
    if (!form.scrcpyDevice) return;
    if (activeDevices.some((device) => device.serial === form.scrcpyDevice)) return;
    setForm((prev) => ({ ...prev, scrcpyDevice: '' }));
  }, [activeDevices, form.scrcpyDevice]);

  useEffect(() => {
    if (!form.recordingDevice) return;
    if (activeDevices.some((device) => device.serial === form.recordingDevice)) return;
    setForm((prev) => ({ ...prev, recordingDevice: '' }));
  }, [activeDevices, form.recordingDevice]);

  useEffect(() => {
    if (localStorage.getItem(FIRST_RUN_KEY) === 'true') return;
    setFirstRunOpen(true);
  }, []);

  useEffect(() => {
    // Only while the modal is actually open: a global handler meant that any
    // Escape (closing a dropdown, clearing focus) permanently hid onboarding.
    if (!firstRunOpen) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setFirstRunOpen(false);
        localStorage.setItem(FIRST_RUN_KEY, 'true');
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [firstRunOpen]);

  // Poll bootstrap progress while the boot overlay is up.
  useEffect(() => {
    if (appReady) return;
    let cancelled = false;
    let timer = 0;
    const tick = async () => {
      const done = await loadBootProgress();
      if (cancelled || done) return;
      timer = window.setTimeout(tick, 700);
    };
    void tick();
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [appReady, loadBootProgress]);

  const closeFirstRunModal = (markSeen = true) => {
    setFirstRunOpen(false);
    if (markSeen) {
      localStorage.setItem(FIRST_RUN_KEY, 'true');
    }
  };

  const updateForm = (field: keyof FormState, value: FormState[keyof FormState]) => {
    setForm((prev) => ({ ...prev, [field]: value }));
  };

  const isSectionCollapsed = useCallback(
    (sectionId: string) => Boolean(collapsedSections[sectionId]),
    [collapsedSections]
  );

  const toggleSection = useCallback((sectionId: string) => {
    setCollapsedSections((prev) => ({ ...prev, [sectionId]: !prev[sectionId] }));
  }, []);

  const openSection = useCallback((sectionId: string) => {
    setCollapsedSections((prev) => (prev[sectionId] ? { ...prev, [sectionId]: false } : prev));
  }, []);

  const sectionContentId = useCallback((sectionId: string) => `${sectionId}-content`, []);

  const parseDeviceAddress = (serial: string) => {
    if (!serial.includes(':')) return null;
    const [ip, port] = serial.split(':');
    if (!ip) return null;
    return { ip, port: port || '5555' };
  };

  const saveDevice = async (name: string, ip: string, port: string) => {
    const response = await apiFetch('/api/devices/save', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, ip, port })
    });
    return response.json();
  };

  const saveDeviceFromForm = async () => {
    const name = form.saveDeviceName.trim();
    const ip = form.saveDeviceIp.trim();
    const port = form.saveDevicePort.trim() || '5555';

    if (!name) {
      notify('error', 'notification_name_required');
      return;
    }

    if (!ip) {
      notify('error', 'notification_address_required');
      return;
    }

    try {
      const data = await saveDevice(name, ip, port);
      if (data.success) {
        notify('success', 'notification_device_saved');
        setForm((prev) => ({
          ...prev,
          saveDeviceName: '',
          saveDeviceIp: '',
          saveDevicePort: '5555'
        }));
        await loadDevices();
      } else {
        notify('error', 'notification_device_save_failed');
      }
    } catch (error) {
      notify('error', 'notification_device_save_failed');
    }
  };

  const fillSaveDeviceForm = (serial: string) => {
    const parsed = parseDeviceAddress(serial);
    if (!parsed) {
      notify('error', 'notification_address_required');
      return;
    }
    setForm((prev) => ({
      ...prev,
      saveDeviceIp: parsed.ip,
      saveDevicePort: parsed.port
    }));
    saveNameRef.current?.focus();
  };

  const handleLanguageChange = async (value: string) => {
    setLang(value);
    await apiFetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ language: value })
    });
    await loadI18n(value);
  };

  const quickConnect = async () => {
    const address = form.quickConnectAddress.trim();
    if (!address) {
      notify('error', 'notification_address_required');
      return;
    }

    try {
      const response = await apiFetch('/api/connect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address })
      });
      const data = await response.json();
      notify(
        data.success ? 'success' : 'error',
        data.success ? 'notification_connected' : 'notification_connect_failed'
      );
      if (data.success) {
        setForm((prev) => ({ ...prev, quickConnectAddress: '' }));
        await loadDevices();
      }
    } catch (error) {
      notify('error', 'notification_connect_failed');
    }
  };

  const connectSaved = async (address: string) => {
    try {
      const response = await apiFetch('/api/connect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address })
      });
      const data = await response.json();
      notify(
        data.success ? 'success' : 'error',
        data.success ? 'notification_connected' : 'notification_connect_failed'
      );
      await loadDevices();
    } catch (error) {
      notify('error', 'notification_connect_failed');
    }
  };

  const disconnectDevice = async (address: string) => {
    try {
      await apiFetch('/api/disconnect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address })
      });
      notify('success', 'notification_disconnected');
      await loadDevices();
    } catch (error) {
      notify('error', 'notification_connect_failed');
    }
  };

  const deleteSaved = async (ip: string, port: string) => {
    if (!(await confirmAction(`${t('device_delete')}?`))) return;
    try {
      await apiFetch(`/api/devices/${ip}/${port}`, { method: 'DELETE' });
      notify('success', 'notification_saved_deleted');
      await loadDevices();
    } catch (error) {
      notify('error', 'notification_connect_failed');
    }
  };

  const pairDevice = async () => {
    const pairAddress = form.pairAddress.trim();
    const pairCode = form.pairCode.trim();

    if (!pairAddress || !pairCode) {
      notify('error', 'notification_address_required');
      return;
    }

    try {
      const response = await apiFetch('/api/pair', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pair_address: pairAddress, pair_code: pairCode })
      });
      const data = await response.json();
      notify(
        data.success ? 'success' : 'error',
        data.success ? 'notification_pair_success' : 'notification_pair_failed'
      );
      if (data.success) {
        setForm((prev) => ({ ...prev, pairAddress: '', pairCode: '' }));
      }
    } catch (error) {
      notify('error', 'notification_pair_failed');
    }
  };

  const enableTcpip = async () => {
    const port = form.tcpipPort.trim() || '5555';

    try {
      const response = await apiFetch('/api/tcpip', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ port })
      });
      const data = await response.json();
      if (data.success && data.ip) {
        notify('success', 'notification_tcpip_success');
        setForm((prev) => ({
          ...prev,
          quickConnectAddress: `${data.ip}:${port}`
        }));
      } else {
        notify('error', 'notification_tcpip_failed');
      }
    } catch (error) {
      notify('error', 'notification_tcpip_failed');
    }
  };

  const applyPreset = (name: string) => {
    const preset = presets.find((item) => item.name === name);
    if (!preset) return;

    setForm((prev) => ({
      ...prev,
      bitrateNum: `${parseInt(preset.bitrate, 10) || 8}`,
      maxsize: `${preset.maxsize}`,
      scrcpyPreset: preset.name
    }));
  };

  const savePreset = async () => {
    const name = form.presetName.trim();
    if (!name) {
      notify('error', 'notification_name_required');
      return;
    }

    const bitrate = `${form.bitrateNum.trim()}M`;
    const maxsize = form.maxsize.trim();

    try {
      const response = await apiFetch('/api/presets', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, bitrate, maxsize })
      });
      const data = await response.json();
      setPresets(data.presets || []);
      setForm((prev) => ({ ...prev, presetName: '' }));
      notify('success', 'notification_preset_saved');
    } catch (error) {
      notify('error', 'notification_preset_failed');
    }
  };

  const removePreset = async (name: string) => {
    try {
      const response = await apiFetch(`/api/presets/${encodeURIComponent(name)}`, { method: 'DELETE' });
      const data = await response.json();
      setPresets(data.presets || []);
      notify('success', 'notification_preset_deleted');
    } catch (error) {
      notify('error', 'notification_preset_failed');
    }
  };

  const resolveAutoConnection = async (serial?: string) => {
    if (!autoConnectionEnabled) return null;
    try {
      const port = form.tcpipPort.trim() || '5555';
      const response = await apiFetch('/api/connection/auto-switch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ serial, port })
      });
      const data = await readJson<{
        success?: boolean;
        recommended?: { serial?: string; type?: string };
        error?: string;
      }>(response);
      if (!response.ok || !data.success || !data.recommended) {
        return null;
      }
      return data.recommended;
    } catch (error) {
      console.warn('auto connection error', error);
      return null;
    }
  };

  const resolveConnectionParams = async (serial: string | undefined, connection: string) => {
    if (connection) {
      return { serial, connection };
    }
    const recommendation = await resolveAutoConnection(serial);
    if (!recommendation) {
      return { serial, connection };
    }
    const nextSerial = recommendation.serial || serial;
    const nextConnection = !nextSerial && !connection && recommendation.type ? recommendation.type : connection;
    return { serial: nextSerial, connection: nextConnection };
  };

  const launchScrcpy = async () => {
    const bitrateValue = form.bitrateNum.trim() || '8';
    const resolved = await resolveConnectionParams(form.scrcpyDevice || undefined, form.connection);
    const params: Record<string, string | boolean> = {
      bitrate: `${bitrateValue}M`,
      maxsize: form.maxsize.trim(),
      keyboard: form.keyboard,
      connection: resolved.connection,
      preset: form.scrcpyPreset,
      stay_awake: form.stayAwake,
      show_touches: form.showTouches,
      turn_screen_off: form.turnScreenOff,
      fullscreen: form.fullscreen,
      no_audio: form.noAudio
    };
    if (resolved.serial) {
      params.serial = resolved.serial;
    }

    try {
      const response = await apiFetch('/api/scrcpy/launch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(params)
      });
      const data = await response.json();
      if (data.success) {
        notify('success', 'notification_scrcpy_started');
        if (data.warning_key) {
          notify('error', data.warning_key);
        }
        // "Keep screen awake" / "Show touches" used to fail silently when adb
        // saw more than one device.
        if (Array.isArray(data.failed_settings) && data.failed_settings.length) {
          notifyMessage(
            'error',
            formatMessage('notification_settings_not_applied', {
              settings: data.failed_settings.join(', ')
            })
          );
        }
      } else {
        notify('error', 'notification_scrcpy_failed');
      }
    } catch (error) {
      notify('error', 'notification_scrcpy_failed');
    }
  };

  const startRecording = async () => {
    if (recordingStatus?.active) return;
    setRecordingLoading(true);
    const bitrateValue = form.bitrateNum.trim() || '8';
    const resolved = await resolveConnectionParams(form.recordingDevice || undefined, form.recordingConnection);
    const params: Record<string, string | boolean> = {
      bitrate: `${bitrateValue}M`,
      maxsize: form.maxsize.trim(),
      keyboard: form.keyboard,
      format: form.recordingFormat,
      audio_source: form.recordingAudioSource,
      show_preview: form.recordingShowPreview,
      stay_awake: form.recordingStayAwake,
      show_touches: form.recordingShowTouches,
      turn_screen_off: form.recordingTurnScreenOff
    };
    if (form.recordingOutputDir.trim()) {
      params.output_dir = form.recordingOutputDir.trim();
    }
    if (form.recordingFilePrefix.trim()) {
      params.file_prefix = form.recordingFilePrefix.trim();
    }
    if (resolved.connection) {
      params.connection = resolved.connection;
    }
    if (resolved.serial) {
      params.serial = resolved.serial;
    }

    try {
      const response = await apiFetch('/api/recording/start', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(params)
      });
      const data = await response.json();
      if (response.ok && data.success) {
        notify('success', 'notification_recording_started');
        if (Array.isArray(data.failed_settings) && data.failed_settings.length) {
          notifyMessage(
            'error',
            formatMessage('notification_settings_not_applied', {
              settings: data.failed_settings.join(', ')
            })
          );
        }
        setRecordingStatus({
          active: true,
          pid: data.pid,
          started_at: data.started_at,
          output_path: data.output_path,
          ...data.settings
        });
      } else {
        notifyMessage('error', data.detail || t('notification_recording_failed'));
      }
    } catch (error) {
      notify('error', 'notification_recording_failed');
    } finally {
      setRecordingLoading(false);
      void loadRecordingStatus();
    }
  };

  const stopRecording = async () => {
    setRecordingLoading(true);
    try {
      const response = await apiFetch('/api/recording/stop', { method: 'POST' });
      const data = await response.json();
      if (data.success) {
        notify('success', 'notification_recording_stopped');
        if (data.warning_key) {
          // scrcpy had to be killed: the container may be missing its index.
          notify('error', data.warning_key);
        }
      } else {
        notifyMessage('error', data.message || t('notification_recording_stop_failed'));
      }
    } catch (error) {
      notify('error', 'notification_recording_stop_failed');
    } finally {
      setRecordingLoading(false);
      void loadRecordingStatus();
    }
  };

  const pickLogsExportDir = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return null;
      return Array.isArray(selected) ? selected[0] : selected;
    } catch (error) {
      console.error('pickLogsExportDir error', error);
      return null;
    }
  }, []);

  const pickDownloadsBaseDir = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return null;
      return Array.isArray(selected) ? selected[0] : selected;
    } catch (error) {
      console.error('pickDownloadsBaseDir error', error);
      return null;
    }
  }, []);

  const pickRecordingDir = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return null;
      return Array.isArray(selected) ? selected[0] : selected;
    } catch (error) {
      console.error('pickRecordingDir error', error);
      return null;
    }
  }, []);

  const saveLogsExportDir = useCallback(
    async (nextDir: string) => {
      setLogsExportDir(nextDir);
      try {
        const response = await apiFetch('/api/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ logs: { export_dir: nextDir } })
        });
        if (!response.ok) {
          throw new Error(`config update failed: ${response.status}`);
        }
      } catch (error) {
        console.error('saveLogsExportDir error', error);
        notify('error', 'notification_config_failed');
      }
    },
    [notify]
  );

  const saveDownloadsBaseDir = useCallback(
    async (nextDir: string) => {
      setDownloadsBaseDir(nextDir);
      try {
        const response = await apiFetch('/api/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ downloads: { base_dir: nextDir } })
        });
        if (!response.ok) {
          throw new Error(`config update failed: ${response.status}`);
        }
      } catch (error) {
        console.error('saveDownloadsBaseDir error', error);
        notify('error', 'notification_config_failed');
      }
    },
    [notify]
  );

  const selectRecordingDir = useCallback(async () => {
    const selected = await pickRecordingDir();
    if (selected) {
      updateForm('recordingOutputDir', selected);
    }
  }, [pickRecordingDir]);

  const selectDownloadsBaseDir = useCallback(async () => {
    const selected = await pickDownloadsBaseDir();
    if (!selected) return null;
    await saveDownloadsBaseDir(selected);
    return selected;
  }, [pickDownloadsBaseDir, saveDownloadsBaseDir]);

  const saveRecordingDefaults = useCallback(
    async (silent = false) => {
      const payload = {
        output_dir: form.recordingOutputDir.trim(),
        format: form.recordingFormat,
        audio_source: form.recordingAudioSource,
        file_prefix: form.recordingFilePrefix.trim() || 'recording',
        show_preview: form.recordingShowPreview,
        stay_awake: form.recordingStayAwake,
        show_touches: form.recordingShowTouches,
        turn_screen_off: form.recordingTurnScreenOff
      };
      setRecordingSaving(true);
      try {
        const response = await apiFetch('/api/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ recording: payload })
        });
        if (!response.ok) {
          throw new Error(`recording config failed: ${response.status}`);
        }
        if (!silent) {
          notify('success', 'notification_recording_defaults_saved');
        }
      } catch (error) {
        if (!silent) {
          notify('error', 'notification_recording_defaults_failed');
        }
      } finally {
        setRecordingSaving(false);
      }
    },
    [
      form.recordingAudioSource,
      form.recordingFilePrefix,
      form.recordingFormat,
      form.recordingOutputDir,
      form.recordingShowPreview,
      form.recordingShowTouches,
      form.recordingStayAwake,
      form.recordingTurnScreenOff,
      notify
    ]
  );

  const selectLogsExportDir = useCallback(async () => {
    const selected = await pickLogsExportDir();
    if (!selected) return null;
    await saveLogsExportDir(selected);
    return selected;
  }, [pickLogsExportDir, saveLogsExportDir]);

  const exportLogs = useCallback(async () => {
    let targetDir = logsExportDir.trim();
    if (!targetDir) {
      const selected = await selectLogsExportDir();
      if (!selected) {
        // Каталог не выбран — экспортировать некуда. Молчать здесь нельзя:
        // пользователь нажал кнопку и ждёт файл.
        notify('error', 'notification_logs_export_failed');
        return;
      }
      targetDir = selected;
    }

    try {
      const response = await apiFetch('/api/logs/export', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ directory: targetDir })
      });
      if (!response.ok) {
        throw new Error(`export failed: ${response.status}`);
      }
      const data = await response.json();
      if (!data.success) {
        throw new Error('export failed');
      }
      notifyMessage('success', formatMessage('notification_logs_exported', { path: data.path || targetDir }));
    } catch (error) {
      console.error('exportLogs error', error);
      notify('error', 'notification_logs_export_failed');
    }
  }, [formatMessage, logsExportDir, notify, notifyMessage, selectLogsExportDir]);

  const loadFullConfig = useCallback(
    async (silent = false) => {
      setConfigLoading(true);
      try {
        const response = await apiFetch('/api/config/full');
        if (!response.ok) {
          throw new Error(`config load failed: ${response.status}`);
        }
        const data = await response.json();
        setConfigDraft(JSON.stringify(data, null, 2));
      } catch (error) {
        console.error('loadFullConfig error', error);
        if (!silent) {
          notify('error', 'notification_config_failed');
        }
      } finally {
        setConfigLoading(false);
      }
    },
    [notify]
  );

  const saveFullConfig = useCallback(async () => {
    let parsed;
    try {
      parsed = JSON.parse(configDraft);
    } catch (error) {
      notify('error', 'notification_config_invalid');
      return;
    }

    setConfigSaving(true);
    try {
      // PUT, not POST: the editor sends the whole config, and a merge could
      // never delete a key the user removed.
      const response = await apiFetch('/api/config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(parsed)
      });
      if (!response.ok) {
        throw new Error(`config save failed: ${response.status}`);
      }
      const data = await response.json();
      const updated = data.config || parsed;
      const nextLang = updated.language || 'en';
      setLang(nextLang);
      setPresets(updated.scrcpy?.presets || []);
      setLogsExportDir(updated.logs?.export_dir || '');
      setDownloadsBaseDir(updated.downloads?.base_dir || '');
      await loadI18n(nextLang);
      setConfigDraft(JSON.stringify(updated, null, 2));
      notify('success', 'notification_config_saved');
    } catch (error) {
      console.error('saveFullConfig error', error);
      notify('error', 'notification_config_failed');
    } finally {
      setConfigSaving(false);
    }
  }, [configDraft, loadI18n, notify]);

  useEffect(() => {
    if (!appReady) return;
    void loadFullConfig(true);
  }, [appReady, loadFullConfig]);

  /** Desktop: download + install + relaunch. Everywhere else: release page. */
  const runSelfUpdate = useCallback(
    async (version: string, notes: string) => {
      const promptText =
        formatMessage('update_available_prompt', { latest: version }) +
        (notes ? `\n\n${notes}` : '');
      if (!(await confirmAction(promptText))) return;

      setUpdateProgress(t('update_downloading_started'));
      try {
        await installUpdate((progress) => {
          setUpdateProgress(
            progress.percent === null
              ? t('update_downloading_started')
              : formatMessage('update_downloading', { percent: String(progress.percent) })
          );
        });
      } catch (error) {
        console.error('installUpdate error', error);
        setUpdateProgress('');
        notify('error', 'notification_update_failed');
        return;
      }

      setUpdateProgress('');
      if (await confirmAction(t('update_restart_prompt'))) {
        await restartApp();
        return;
      }
      notify('success', 'notification_update_applied');
    },
    [formatMessage, notify, t]
  );

  const offerReleasePage = useCallback(
    async (version: string, notes: string, releaseUrl: string) => {
      // Distinct wording: this path opens GitHub, it does not install anything.
      const promptText =
        formatMessage('update_open_release_prompt', { latest: version }) +
        (notes ? `\n\n${notes}` : '');
      if (await confirmAction(promptText)) {
        await openReleasePage(releaseUrl);
      }
    },
    [formatMessage]
  );

  const checkUpdates = useCallback(async () => {
    // The backend check works everywhere and gives us the release URL for the
    // fallback path, so it runs first regardless of build type.
    let backend: {
      update_available?: boolean;
      latest?: string;
      release?: { body?: string };
      release_url?: string;
    };
    try {
      const response = await apiFetch('/api/update/check');
      backend = await readJson(response);
    } catch (error) {
      console.error('checkUpdates error', error);
      notify('error', 'notification_update_failed');
      return;
    }

    const notes = (backend.release?.body || '').trim();
    const releaseUrl = backend.release_url || '';

    if (canSelfUpdate()) {
      try {
        const update = await checkForUpdate();
        if (update) {
          await runSelfUpdate(update.version, update.notes || notes);
          return;
        }
        if (!backend.update_available) {
          notify('success', 'notification_update_latest');
          return;
        }
        // The signed feed does not know about it yet — send them to GitHub.
        await offerReleasePage(backend.latest || '', notes, releaseUrl);
        return;
      } catch (error) {
        // No signing key configured yet, feed unreachable, portable build...
        console.warn('self-update unavailable, falling back to release page', error);
      }
    }

    if (backend.update_available) {
      await offerReleasePage(backend.latest || '', notes, releaseUrl);
    } else {
      notify('success', 'notification_update_latest');
    }
  }, [notify, offerReleasePage, runSelfUpdate]);

  const serviceSerial = () => form.scrcpyDevice || (activeDevices.length === 1 ? activeDevices[0].serial : undefined);

  // Service Menu: список команд берём с бэкенда, а не дублируем во фронте.
  const loadServiceCommands = useCallback(async () => {
    try {
      const response = await apiFetch('/api/service/commands');
      if (!response.ok) return;
      const data = await readJson<{ commands?: string[] }>(response);
      if (Array.isArray(data.commands)) {
        setServiceCommands(data.commands);
      }
    } catch (error) {
      console.error('loadServiceCommands error', error);
    }
  }, []);

  // Service Menu: выполнить предопределённую команду
  const runServiceCommand = async (command: string) => {
    setServiceLoading(true);
    setServiceOutput('');
    try {
      const serial = serviceSerial();
      const url = `/api/service/${command}${serial ? `?serial=${encodeURIComponent(serial)}` : ''}`;
      const response = await apiFetch(url, { method: 'POST' });
      const data = await readJson<{ output?: string; error?: string; detail?: string }>(response);
      if (!response.ok) {
        setServiceOutput(fileErrorMessage(data, `Request failed: ${response.status}`));
        return;
      }
      setServiceOutput(data.output || data.error || 'No output');
    } catch (error) {
      setServiceOutput(`Error: ${error}`);
    } finally {
      setServiceLoading(false);
    }
  };

  // Service Menu: выполнить кастомную команду
  const runCustomCommand = async (confirmed = false) => {
    if (!serviceCommand.trim()) return;
    setServiceLoading(true);
    setServiceOutput('');
    try {
      const response = await apiFetch('/api/service/custom', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command: serviceCommand,
          serial: serviceSerial() || null,
          confirm: confirmed
        })
      });
      const data = await readJson<{ output?: string; error?: string; detail?: string }>(response);
      if (response.status === 403 && !confirmed) {
        // Бэкенд просит подтверждения: команда выходит за пределы диагностики.
        const detail = typeof data.detail === 'string' ? data.detail : '';
        setServiceLoading(false);
        const accepted = await confirmAction(`${t('service_confirm_dangerous')}\n\n${detail}`);
        if (accepted) {
          await runCustomCommand(true);
        }
        return;
      }
      if (!response.ok) {
        setServiceOutput(fileErrorMessage(data, `Request failed: ${response.status}`));
        return;
      }
      setServiceOutput(data.output || data.error || 'No output');
    } catch (error) {
      setServiceOutput(`Error: ${error}`);
    } finally {
      setServiceLoading(false);
    }
  };

  // Screenshots Gallery: загрузить список
  const loadScreenshots = async (
    nextPage: number = screenshotsPage,
    nextPageSize: number = screenshotsPageSize
  ) => {
    setScreenshotsLoading(true);
    try {
      const url = `/api/screenshots?page=${nextPage}&page_size=${nextPageSize}`;
      const response = await apiFetch(url);
      const data = await readJson<{
        screenshots?: Screenshot[];
        count?: number;
        total_count?: number;
        page?: number;
        page_size?: number;
        total_pages?: number;
        error?: string;
      }>(response);
      if (!response.ok) {
        setScreenshots([]);
        setScreenshotsTotal(0);
        setScreenshotsTotalPages(1);
        setScreenshotsPage(1);
        return;
      }
      const totalCount = typeof data.total_count === 'number' ? data.total_count : data.count || 0;
      const pageSize = typeof data.page_size === 'number' ? data.page_size : nextPageSize;
      const totalPages =
        typeof data.total_pages === 'number'
          ? data.total_pages
          : Math.max(1, Math.ceil(totalCount / Math.max(pageSize, 1)));
      const page = typeof data.page === 'number' ? data.page : nextPage;
      setScreenshots(data.screenshots || []);
      setScreenshotsTotal(totalCount);
      setScreenshotsPageSize(pageSize);
      setScreenshotsTotalPages(totalPages);
      setScreenshotsPage(page);
    } catch {
      setScreenshots([]);
      setScreenshotsTotal(0);
      setScreenshotsTotalPages(1);
      setScreenshotsPage(1);
    } finally {
      setScreenshotsLoading(false);
    }
  };

  // Screenshots Gallery: сделать скриншот
  const takeScreenshot = async (caption?: string) => {
    setTakingScreenshot(true);
    try {
      const serial = form.scrcpyDevice || undefined;
      const params = new URLSearchParams();
      if (serial) params.set('serial', serial);
      if (caption) params.set('caption', caption);
      const url = `/api/screenshots/take${params.toString() ? `?${params.toString()}` : ''}`;
      const response = await apiFetch(url, { method: 'POST' });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok) {
        notify('error', 'notification_screenshot_failed');
        return false;
      }
      if (data.success) {
        notify('success', 'notification_screenshot_taken');
        await loadScreenshots(1, screenshotsPageSize);
        return true;
      }
      return false;
    } catch {
      notify('error', 'notification_screenshot_failed');
      return false;
    } finally {
      setTakingScreenshot(false);
    }
  };

  const saveAutoConnection = async (enabled: boolean) => {
    const previous = autoConnectionEnabled;
    setAutoConnectionEnabled(enabled);
    setAutoConnectionSaving(true);
    try {
      const response = await apiFetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ connection_optimizer: { auto_switch: enabled } })
      });
      if (!response.ok) {
        throw new Error(`config update failed: ${response.status}`);
      }
    } catch (error) {
      setAutoConnectionEnabled(previous);
      notify('error', 'notification_config_failed');
    } finally {
      setAutoConnectionSaving(false);
    }
  };

  const downloadScreenshot = async (id: string) => {
    try {
      // Скачивание через blob и `<a download>` внутри WebView молча ничего не
      // делает, поэтому файл кладёт на диск бэкенд.
      const response = await apiFetch(`/api/screenshots/${id}/save`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({})
      });
      const data = await readJson<{ success?: boolean; path?: string; detail?: string }>(response);
      if (!response.ok || !data.success) {
        notifyMessage('error', fileErrorMessage(data, t('notification_screenshot_download_failed')));
        return;
      }
      notifyMessage(
        'success',
        formatMessage('notification_screenshot_saved_to', { path: data.path || '' })
      );
    } catch (error) {
      console.error('downloadScreenshot error', error);
      notify('error', 'notification_screenshot_download_failed');
    }
  };

  const deleteScreenshot = async (id: string) => {
    setScreenshotsLoading(true);
    try {
      const response = await apiFetch(`/api/screenshots/${id}`, { method: 'DELETE' });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok || !data.success) {
        notify('error', 'notification_screenshot_delete_failed');
        return false;
      }
      notify('success', 'notification_screenshot_deleted');
      await loadScreenshots(screenshotsPage, screenshotsPageSize);
      return true;
    } catch {
      notify('error', 'notification_screenshot_delete_failed');
      return false;
    } finally {
      setScreenshotsLoading(false);
    }
  };

  const updateScreenshotCaption = async (id: string, caption: string) => {
    try {
      const response = await apiFetch(`/api/screenshots/${id}/caption`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ caption })
      });
      const data = await readJson<{ success?: boolean; screenshot?: Screenshot; error?: string }>(response);
      if (!response.ok || !data.success) {
        notify('error', 'notification_screenshot_caption_failed');
        return false;
      }
      setScreenshots((prev) =>
        prev.map((item) => (item.id === id ? { ...item, caption } : item))
      );
      notify('success', 'notification_screenshot_caption_saved');
      return true;
    } catch (error) {
      console.error('updateScreenshotCaption error', error);
      notify('error', 'notification_screenshot_caption_failed');
      return false;
    }
  };

  const changeScreenshotsPage = (page: number) => {
    const maxPage = Math.max(1, screenshotsTotalPages || 1);
    const nextPage = Math.min(Math.max(1, page), maxPage);
    void loadScreenshots(nextPage, screenshotsPageSize);
  };

  const changeScreenshotsPageSize = (size: number) => {
    const nextSize = Math.max(1, size);
    setScreenshotsPageSize(nextSize);
    void loadScreenshots(1, nextSize);
  };

  // File Manager: загрузить директорию
  const getFileSerial = () => {
    if (form.scrcpyDevice) return form.scrcpyDevice;
    if (activeDevices.length === 1) return activeDevices[0].serial;
    return undefined;
  };

  const fileErrorMessage = (data: unknown, fallback: string) => {
    if (!data || typeof data !== 'object') return fallback;
    const record = data as { detail?: unknown; error?: unknown; message?: unknown };
    if (typeof record.detail === 'string') return record.detail;
    if (typeof record.error === 'string') return record.error;
    if (typeof record.message === 'string') return record.message;
    return fallback;
  };

  const withFileSerial = (url: string) => {
    const serial = getFileSerial();
    if (!serial) return url;
    const joiner = url.includes('?') ? '&' : '?';
    return `${url}${joiner}serial=${encodeURIComponent(serial)}`;
  };

  const loadFiles = async (
    path: string,
    nextPage: number = filesPage,
    nextPageSize: number = filesPageSize
  ) => {
    setFilesLoading(true);
    setFilesError('');
    try {
      const serial = getFileSerial();
      if (!serial && activeDevices.length > 1) {
        const message = t('notification_files_select_device');
        setFilesError(message);
        notifyMessage('error', message);
        setFiles([]);
        return;
      }
      if (!serial && activeDevices.length === 0) {
        const message = t('notification_files_no_device');
        setFilesError(message);
        notifyMessage('error', message);
        setFiles([]);
        return;
      }
      const url = withFileSerial(
        `/api/files/list?path=${encodeURIComponent(path)}&page=${nextPage}&page_size=${nextPageSize}`
      );
      const response = await apiFetch(url);
      const data = await readJson<{
        files?: FileEntry[];
        total_count?: number;
        page?: number;
        page_size?: number;
        total_pages?: number;
        error?: string;
      }>(response);
      if (!response.ok) {
        const message = fileErrorMessage(data, formatMessage('notification_files_list_failed', { path }));
        setFilesError(message);
        notifyMessage('error', message);
        setFiles([]);
        setFilesTotal(0);
        setFilesTotalPages(1);
        setFilesPage(1);
        return;
      }
      const totalCount = typeof data.total_count === 'number' ? data.total_count : data.files?.length || 0;
      const pageSize = typeof data.page_size === 'number' ? data.page_size : nextPageSize;
      const totalPages =
        typeof data.total_pages === 'number'
          ? data.total_pages
          : Math.max(1, Math.ceil(totalCount / Math.max(pageSize, 1)));
      const page = typeof data.page === 'number' ? data.page : nextPage;
      setFiles(data.files || []);
      setCurrentPath(path);
      setFilesTotal(totalCount);
      setFilesPage(page);
      setFilesPageSize(pageSize);
      setFilesTotalPages(totalPages);
    } catch {
      const message = formatMessage('notification_files_list_failed', { path });
      setFilesError(message);
      notifyMessage('error', message);
      setFiles([]);
      setFilesTotal(0);
      setFilesTotalPages(1);
      setFilesPage(1);
    } finally {
      setFilesLoading(false);
    }
  };

  // File Manager: навигация в директорию
  const navigateToDir = (path: string) => {
    void loadFiles(path, 1, filesPageSize);
  };

  const refreshFiles = () => {
    void loadFiles(currentPath, filesPage, filesPageSize);
  };

  const changeFilesPage = (page: number) => {
    const maxPage = Math.max(1, filesTotalPages || 1);
    const nextPage = Math.min(Math.max(1, page), maxPage);
    void loadFiles(currentPath, nextPage, filesPageSize);
  };

  const changeFilesPageSize = (size: number) => {
    const nextSize = Math.max(1, size);
    setFilesPageSize(nextSize);
    void loadFiles(currentPath, 1, nextSize);
  };

  const downloadFile = async (entry: FileEntry) => {
    if (entry.is_dir) return;
    try {
      const response = await apiFetch(withFileSerial('/api/files/pull'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: entry.path })
      });
      const data = await readJson<{ success?: boolean; path?: string; detail?: string }>(response);
      if (!response.ok || !data.success) {
        const message = fileErrorMessage(
          data,
          formatMessage('notification_file_download_failed', { name: entry.name })
        );
        notifyMessage('error', message);
        return;
      }
      notifyMessage('success', formatMessage('notification_file_saved_to', { path: data.path || '' }));
    } catch (error) {
      console.error('downloadFile error', error);
      notifyMessage('error', formatMessage('notification_file_download_failed', { name: entry.name }));
    }
  };

  const deleteFile = async (entry: FileEntry) => {
    setFilesBusy(true);
    try {
      const url = withFileSerial(`/api/files/delete?path=${encodeURIComponent(entry.path)}`);
      const response = await apiFetch(url, { method: 'DELETE' });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok || !data.success) {
        const message = fileErrorMessage(
          data,
          formatMessage('notification_file_delete_failed', { name: entry.name })
        );
        notifyMessage('error', message);
        return;
      }
      notifyMessage('success', formatMessage('notification_file_deleted', { name: entry.name }));
      await loadFiles(currentPath, filesPage, filesPageSize);
    } catch (error) {
      console.error('deleteFile error', error);
      notifyMessage('error', formatMessage('notification_file_delete_failed', { name: entry.name }));
    } finally {
      setFilesBusy(false);
    }
  };

  const createDirectory = async (path: string) => {
    setFilesBusy(true);
    try {
      const url = withFileSerial('/api/files/mkdir');
      const response = await apiFetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path })
      });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok || !data.success) {
        const message = fileErrorMessage(
          data,
          formatMessage('notification_folder_create_failed', { path })
        );
        notifyMessage('error', message);
        return;
      }
      notifyMessage('success', formatMessage('notification_folder_created', { path }));
      await loadFiles(currentPath, filesPage, filesPageSize);
    } catch (error) {
      console.error('createDirectory error', error);
      notifyMessage('error', formatMessage('notification_folder_create_failed', { path }));
    } finally {
      setFilesBusy(false);
    }
  };

  const moveEntry = async (source: string, destination: string) => {
    setFilesBusy(true);
    try {
      const url = withFileSerial('/api/files/move');
      const response = await apiFetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source, destination })
      });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok || !data.success) {
        const message = fileErrorMessage(data, formatMessage('notification_file_move_failed', { source }));
        notifyMessage('error', message);
        return;
      }
      notifyMessage('success', formatMessage('notification_file_moved', { destination }));
      await loadFiles(currentPath, filesPage, filesPageSize);
    } catch (error) {
      console.error('moveEntry error', error);
      notifyMessage('error', formatMessage('notification_file_move_failed', { source }));
    } finally {
      setFilesBusy(false);
    }
  };

  const readFile = async (path: string) => {
    const url = withFileSerial(`/api/files/read?path=${encodeURIComponent(path)}`);
    const response = await apiFetch(url);
    const data = await readJson<{
      content?: string;
      truncated?: boolean;
      is_binary?: boolean;
      error?: string;
    }>(response);
    if (!response.ok) {
      throw new Error(fileErrorMessage(data, formatMessage('notification_file_read_failed', { path })));
    }
    return {
      content: data.content || '',
      truncated: Boolean(data.truncated),
      isBinary: Boolean(data.is_binary)
    };
  };

  const writeFile = async (path: string, content: string) => {
    setFilesBusy(true);
    try {
      const url = withFileSerial('/api/files/write');
      const response = await apiFetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path, content })
      });
      const data = await readJson<{ success?: boolean; error?: string }>(response);
      if (!response.ok || !data.success) {
        const message = fileErrorMessage(data, formatMessage('notification_file_write_failed', { path }));
        notifyMessage('error', message);
        return false;
      }
      notifyMessage('success', formatMessage('notification_file_written', { path }));
      await loadFiles(currentPath, filesPage, filesPageSize);
      return true;
    } catch (error) {
      console.error('writeFile error', error);
      notifyMessage('error', formatMessage('notification_file_write_failed', { path }));
      return false;
    } finally {
      setFilesBusy(false);
    }
  };

  /**
   * Итог загрузки: счётчик, уведомление и обновление списка.
   *
   * Общий хвост для двух путей — multipart в вебе и путей на диске в десктопе.
   */
  const finishUpload = async (successCount: number, total: number, destination: string) => {
    if (successCount) {
      notifyMessage(
        'success',
        formatMessage('notification_files_uploaded', {
          count: String(successCount),
          total: String(total)
        })
      );
    }
    const nextPage = destination === currentPath ? filesPage : 1;
    await loadFiles(destination, nextPage, filesPageSize);
  };

  /**
   * Загрузка по путям на диске — десктопный путь.
   *
   * Через IPC содержимое файла не передать, поэтому фронтенд отдаёт Rust путь,
   * а `adb push` читает файл сам. Пути приходят либо из системного диалога,
   * либо из события перетаскивания Tauri.
   */
  const uploadPaths = async (paths: string[], destination: string) => {
    if (!paths.length) return;
    setFilesBusy(true);
    let successCount = 0;
    try {
      for (const path of paths) {
        const url = withFileSerial(
          `/api/files/upload?destination=${encodeURIComponent(destination)}`
        );
        const response = await apiFetch(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ source: path })
        });
        const data = await readJson<{ success?: boolean; detail?: string }>(response);
        if (response.ok && data.success) {
          successCount += 1;
        } else {
          const name = path.split(/[\\/]/).pop() || path;
          notifyMessage('error', fileErrorMessage(data, formatMessage('notification_file_upload_failed', { name })));
        }
      }
      await finishUpload(successCount, paths.length, destination);
    } catch (error) {
      console.error('uploadPaths error', error);
      notify('error', 'notification_upload_failed');
    } finally {
      setFilesBusy(false);
    }
  };

  /** Системный диалог выбора файлов — кнопка «загрузить» в десктопе. */
  const pickUpload = async (destination: string) => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ multiple: true });
      const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
      await uploadPaths(paths, destination);
    } catch (error) {
      console.error('pickUpload error', error);
      notify('error', 'notification_upload_failed');
    }
  };

  const scrcpyOptions = useMemo(() => {
    return activeDevices.map((device) => ({
      value: device.serial,
      label: `${device.serial} (${device.status})`
    }));
  }, [activeDevices]);

  const recordingPreviewName = useMemo(() => {
    const prefix = form.recordingFilePrefix.trim() || 'recording';
    return `${prefix}_YYYYMMDD_HHMMSS.${form.recordingFormat}`;
  }, [form.recordingFilePrefix, form.recordingFormat]);

  const recordingPreviewPath = useMemo(() => {
    const joinPath = (root: string, segment: string) => {
      const separator = root.includes('\\') ? '\\' : '/';
      if (root.endsWith('\\') || root.endsWith('/')) {
        return `${root}${segment}`;
      }
      return `${root}${separator}${segment}`;
    };

    const base = form.recordingOutputDir.trim();
    if (base) {
      return joinPath(base, recordingPreviewName);
    }

    const downloadsRoot = downloadsBaseDir.trim();
    if (!downloadsRoot) return recordingPreviewName;

    const videoRoot = joinPath(downloadsRoot, 'video');
    return joinPath(videoRoot, recordingPreviewName);
  }, [downloadsBaseDir, form.recordingOutputDir, recordingPreviewName]);

  const recordingElapsed = useMemo(() => {
    if (!recordingStatus?.active || !recordingStatus.started_at) return null;
    const start = new Date(recordingStatus.started_at).getTime();
    if (Number.isNaN(start)) return null;
    return formatDuration(recordingNow - start);
  }, [recordingStatus?.active, recordingStatus?.started_at, recordingNow]);

  const activePage = useMemo(() => getPageForSection(activeSection), [activeSection]);

  // currentPath is deliberately not a dependency: loadFiles sets it on success,
  // so listing a folder used to fire the same `adb shell ls` twice.
  // Navigation, pagination and refresh call loadFiles directly.
  useEffect(() => {
    if (!appReady || activePage !== 'files') return;
    void loadFiles(currentPath, 1, filesPageSize);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appReady, activePage, form.scrcpyDevice]);

  // The gallery is independent of the file tree — reloading it on every
  // folder change was pure waste.
  useEffect(() => {
    if (!appReady || activePage !== 'files') return;
    void loadScreenshots(screenshotsPage, screenshotsPageSize);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appReady, activePage]);

  useEffect(() => {
    if (!appReady) return;
    void loadServiceCommands();
  }, [appReady, loadServiceCommands]);

  useEffect(() => {
    const getHashSection = () => window.location.hash.replace('#', '').trim();
    const initial = getHashSection();
    if (initial) {
      setActiveSection(initial);
    }

    const handleHashChange = () => {
      const next = getHashSection();
      if (next) {
        setActiveSection(next);
      }
    };

    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  const toggleMenuGroup = useCallback((groupId: string) => {
    setExpandedMenuGroups((prev) => ({ ...prev, [groupId]: !prev[groupId] }));
  }, []);

  const navigateToSection = useCallback((sectionId: string) => {
    setActiveSection(sectionId);
    openSection(sectionId);
    setSidebarMobileOpen(false);
    window.location.hash = sectionId;
    window.requestAnimationFrame(() => {
      const element = document.getElementById(sectionId);
      if (element) {
        element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  }, [openSection]);

  const themeLabel = themePreference === 'auto'
    ? t('theme_system') || 'System'
    : theme === 'dark' ? t('theme_dark') : t('theme_light');
  return (
    <div className="min-h-screen">
      <div className="background">
        <div className="orb orb-1" />
        <div className="orb orb-2" />
        <div className="grid-overlay" />
      </div>

      <Sidebar
        t={t}
        activeSection={activeSection}
        sidebarOpen={sidebarOpen}
        sidebarMobileOpen={sidebarMobileOpen}
        onToggleSidebar={() => setSidebarOpen((prev) => !prev)}
        onOpenMobile={() => setSidebarMobileOpen(true)}
        onCloseMobile={() => setSidebarMobileOpen(false)}
        expandedMenuGroups={expandedMenuGroups}
        onToggleMenuGroup={toggleMenuGroup}
        onNavigate={navigateToSection}
        wsConnected={wsConnected}
        theme={theme}
        themePreference={themePreference}
        onThemePreferenceChange={handleThemePreferenceChange}
        onToggleTheme={toggleTheme}
        themeLabel={themeLabel}
      />

      <div
        className={cn(
          'transition-all duration-300',
          'md:ml-[260px]',
          !sidebarOpen && 'md:ml-[60px]'
        )}
      >
        <RecordingBanner
          recordingStatus={recordingStatus}
          recordingElapsed={recordingElapsed}
          recordingLoading={recordingLoading}
          onStop={() => void stopRecording()}
          t={t}
        />

        <div className="mx-auto flex w-full max-w-[1220px] flex-col gap-7">
          <Header
            t={t}
            lang={lang}
            languages={languages}
            onLanguageChange={(value) => void handleLanguageChange(value)}
          />

          <main className="flex flex-col gap-7">
            {activePage === 'home' && (
              <HomePage
                t={t}
                activeSection={activeSection}
                isSectionCollapsed={isSectionCollapsed}
                toggleSection={toggleSection}
                sectionContentId={sectionContentId}
                form={form}
                setForm={setForm}
                updateForm={updateForm}
                presets={presets}
                activeDevices={activeDevices}
                savedDevices={savedDevices}
                devicesLoading={devicesLoading}
                savedLoading={savedLoading}
                saveNameRef={saveNameRef}
                saveDeviceFromForm={saveDeviceFromForm}
                fillSaveDeviceForm={fillSaveDeviceForm}
                connectSaved={connectSaved}
                disconnectDevice={disconnectDevice}
                deleteSaved={deleteSaved}
                quickConnect={quickConnect}
                pairDevice={pairDevice}
                enableTcpip={enableTcpip}
                applyPreset={applyPreset}
                savePreset={savePreset}
                removePreset={removePreset}
                launchScrcpy={launchScrcpy}
                startRecording={startRecording}
                stopRecording={stopRecording}
                autoConnectionEnabled={autoConnectionEnabled}
                autoConnectionSaving={autoConnectionSaving}
                onAutoConnectionChange={saveAutoConnection}
                selectRecordingDir={selectRecordingDir}
                saveRecordingDefaults={saveRecordingDefaults}
                recordingStatus={recordingStatus}
                recordingLoading={recordingLoading}
                recordingSaving={recordingSaving}
                recordingPreviewPath={recordingPreviewPath}
                scrcpyOptions={scrcpyOptions}
                logsExportDir={logsExportDir}
                setLogsExportDir={setLogsExportDir}
                saveLogsExportDir={saveLogsExportDir}
                selectLogsExportDir={selectLogsExportDir}
                downloadsBaseDir={downloadsBaseDir}
                setDownloadsBaseDir={setDownloadsBaseDir}
                saveDownloadsBaseDir={saveDownloadsBaseDir}
                selectDownloadsBaseDir={selectDownloadsBaseDir}
                exportLogs={exportLogs}
                checkUpdates={checkUpdates}
                updateProgress={updateProgress}
                configDraft={configDraft}
                setConfigDraft={setConfigDraft}
                configLoading={configLoading}
                configSaving={configSaving}
                loadFullConfig={loadFullConfig}
                saveFullConfig={saveFullConfig}
              />
            )}
            {activePage === 'service-menu' && (
              <ServiceMenuPage
                t={t}
                activeSection={activeSection}
                isSectionCollapsed={isSectionCollapsed}
                toggleSection={toggleSection}
                serviceLoading={serviceLoading}
                serviceCommand={serviceCommand}
                setServiceCommand={(value) => setServiceCommand(value)}
                runServiceCommand={runServiceCommand}
                runCustomCommand={() => void runCustomCommand()}
                serviceOutput={serviceOutput}
                serviceCommands={serviceCommands}
              />
            )}
            {activePage === 'automation' && (
              <AutomationPage
                t={t}
                activeSection={activeSection}
              />
            )}
            {activePage === 'files' && (
              <FilesPage
                t={t}
                formatMessage={formatMessage}
                activeSection={activeSection}
                isSectionCollapsed={isSectionCollapsed}
                toggleSection={toggleSection}
                screenshots={screenshots}
                screenshotsLoading={screenshotsLoading}
                takingScreenshot={takingScreenshot}
                loadScreenshots={loadScreenshots}
                takeScreenshot={takeScreenshot}
                downloadScreenshot={downloadScreenshot}
                deleteScreenshot={deleteScreenshot}
                updateScreenshotCaption={updateScreenshotCaption}
                screenshotsPage={screenshotsPage}
                screenshotsPageSize={screenshotsPageSize}
                screenshotsTotal={screenshotsTotal}
                screenshotsTotalPages={screenshotsTotalPages}
                onScreenshotsPageChange={changeScreenshotsPage}
                onScreenshotsPageSizeChange={changeScreenshotsPageSize}
                currentPath={currentPath}
                files={files}
                filesLoading={filesLoading}
                filesBusy={filesBusy}
                filesError={filesError}
                refreshFiles={refreshFiles}
                navigateToDir={navigateToDir}
                downloadFile={downloadFile}
                deleteFile={deleteFile}
                createDirectory={createDirectory}
                moveEntry={moveEntry}
                readFile={readFile}
                writeFile={writeFile}
                pickUpload={pickUpload}
                filesPage={filesPage}
                filesPageSize={filesPageSize}
                filesTotal={filesTotal}
                filesTotalPages={filesTotalPages}
                onFilesPageChange={changeFilesPage}
                onFilesPageSizeChange={changeFilesPageSize}
              />
            )}
          </main>

          <Footer t={t} version={appVersion} />
        </div>
      </div>

      <Notifications notifications={notifications} />
      <FirstRunModal open={firstRunOpen} onClose={closeFirstRunModal} t={t} />
      <BootOverlay
        ready={appReady}
        timedOut={bootTimedOut}
        progress={bootProgress}
        onRetry={() => void initializeApp()}
      />
    </div>
  );
}

export default App;
