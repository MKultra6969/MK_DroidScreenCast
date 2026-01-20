import type { Dispatch, RefObject, SetStateAction } from 'react';
import {
  Activity,
  Bookmark,
  ChevronDown,
  Download,
  FolderOpen,
  Key,
  Mic,
  Monitor,
  PhoneOff,
  Play,
  PlugZap,
  Power,
  PowerOff,
  RefreshCw,
  Save,
  SaveOff,
  ShieldCheck,
  Sparkles,
  StopCircle,
  Trash2,
  Usb,
  Video,
  Wifi,
  Zap
} from 'lucide-react';
import { cn } from '../../utils';
import { delayStyle } from '../../lib/style';
import type { Device, FormState, Preset, RecordingStatus, SavedDevice } from '../../types/app';

type HomePageProps = {
  t: (key: string) => string;
  activeSection: string;
  isSectionCollapsed: (sectionId: string) => boolean;
  toggleSection: (sectionId: string) => void;
  sectionContentId: (sectionId: string) => string;
  form: FormState;
  setForm: Dispatch<SetStateAction<FormState>>;
  updateForm: (field: keyof FormState, value: FormState[keyof FormState]) => void;
  presets: Preset[];
  activeDevices: Device[];
  savedDevices: SavedDevice[];
  devicesLoading: boolean;
  savedLoading: boolean;
  saveNameRef: RefObject<HTMLInputElement | null>;
  saveDeviceFromForm: () => void | Promise<void>;
  fillSaveDeviceForm: (serial: string) => void;
  connectSaved: (address: string) => void | Promise<void>;
  disconnectDevice: (address: string) => void | Promise<void>;
  deleteSaved: (ip: string, port: string) => void | Promise<void>;
  quickConnect: () => void | Promise<void>;
  pairDevice: () => void | Promise<void>;
  enableTcpip: () => void | Promise<void>;
  applyPreset: (name: string) => void;
  savePreset: () => void | Promise<void>;
  removePreset: (name: string) => void | Promise<void>;
  launchScrcpy: () => void | Promise<void>;
  startRecording: () => void | Promise<void>;
  stopRecording: () => void | Promise<void>;
  autoConnectionEnabled: boolean;
  autoConnectionSaving: boolean;
  onAutoConnectionChange: (enabled: boolean) => void | Promise<void>;
  selectRecordingDir: () => void | Promise<void>;
  saveRecordingDefaults: (silent?: boolean) => void | Promise<void>;
  recordingStatus: RecordingStatus | null;
  recordingLoading: boolean;
  recordingSaving: boolean;
  recordingPreviewPath: string;
  scrcpyOptions: Array<{ value: string; label: string }>;
  logsExportDir: string;
  setLogsExportDir: Dispatch<SetStateAction<string>>;
  saveLogsExportDir: (nextDir: string) => void | Promise<void>;
  selectLogsExportDir: () => void | Promise<string | null>;
  downloadsBaseDir: string;
  setDownloadsBaseDir: Dispatch<SetStateAction<string>>;
  saveDownloadsBaseDir: (nextDir: string) => void | Promise<void>;
  selectDownloadsBaseDir: () => void | Promise<string | null>;
  downloadLogs: () => void | Promise<void>;
  exportLogs: () => void | Promise<void>;
  checkUpdates: () => void | Promise<void>;
  configDraft: string;
  setConfigDraft: Dispatch<SetStateAction<string>>;
  configLoading: boolean;
  configSaving: boolean;
  loadFullConfig: (silent?: boolean) => void | Promise<void>;
  saveFullConfig: () => void | Promise<void>;
};

export function HomePage({
  t,
  activeSection,
  isSectionCollapsed,
  toggleSection,
  sectionContentId,
  form,
  setForm,
  updateForm,
  presets,
  activeDevices,
  savedDevices,
  devicesLoading,
  savedLoading,
  saveNameRef,
  saveDeviceFromForm,
  fillSaveDeviceForm,
  connectSaved,
  disconnectDevice,
  deleteSaved,
  quickConnect,
  pairDevice,
  enableTcpip,
  applyPreset,
  savePreset,
  removePreset,
  launchScrcpy,
  startRecording,
  stopRecording,
  autoConnectionEnabled,
  autoConnectionSaving,
  onAutoConnectionChange,
  selectRecordingDir,
  saveRecordingDefaults,
  recordingStatus,
  recordingLoading,
  recordingSaving,
  recordingPreviewPath,
  scrcpyOptions,
  logsExportDir,
  setLogsExportDir,
  saveLogsExportDir,
  selectLogsExportDir,
  downloadsBaseDir,
  setDownloadsBaseDir,
  saveDownloadsBaseDir,
  selectDownloadsBaseDir,
  downloadLogs,
  exportLogs,
  checkUpdates,
  configDraft,
  setConfigDraft,
  configLoading,
  configSaving,
  loadFullConfig,
  saveFullConfig
}: HomePageProps) {
  const sectionToggleLabel = (collapsed: boolean) =>
    collapsed ? 'Expand section' : 'Collapse section';
  const sectionHighlightClass = (sectionId: string) =>
    activeSection === sectionId
      ? 'ring-2 ring-[var(--md-sys-color-primary)] ring-offset-2 ring-offset-[var(--md-sys-color-background)]'
      : '';
  const sectionToggleClassName = cn(
    'inline-flex items-center justify-center rounded-full border border-[var(--md-sys-color-outline-variant)] p-1.5',
    'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface-variant)]',
    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
  );
  const sectionToggleIconClass = (collapsed: boolean) =>
    cn('h-4 w-4 transition-transform', collapsed ? '-rotate-90' : 'rotate-0');
  const faqCollapsed = isSectionCollapsed('faq');
  const recordingCollapsed = isSectionCollapsed('recording');
  const activeDevicesCollapsed = isSectionCollapsed('active-devices');
  const savedDevicesCollapsed = isSectionCollapsed('saved-devices');
  const quickConnectCollapsed = isSectionCollapsed('quick-connect');
  const pairingCollapsed = isSectionCollapsed('pairing');
  const usbWifiCollapsed = isSectionCollapsed('usb-wifi');
  const scrcpyCollapsed = isSectionCollapsed('scrcpy');
  const presetsCollapsed = isSectionCollapsed('presets');
  const diagnosticsCollapsed = isSectionCollapsed('diagnostics');
  const configCollapsed = isSectionCollapsed('config');

  return (
    <>
                <section
                  id="faq"
                  className={cn(
                    'reveal flex flex-col gap-3 rounded-[30px] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container-low)] shadow-[var(--shadow-1)]',
                    'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
                    sectionHighlightClass('faq')
                  )}
                  style={delayStyle(80)}
                >
              <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                <div className="flex items-center gap-3">
                  <Sparkles className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                  <h2 className="font-display text-lg font-semibold">{t('section_faq')}</h2>
                </div>
                <button
                  className={sectionToggleClassName}
                  type="button"
                  onClick={() => toggleSection('faq')}
                  aria-expanded={!faqCollapsed}
                  aria-controls={sectionContentId('faq')}
                  aria-label={sectionToggleLabel(faqCollapsed)}
                  title={sectionToggleLabel(faqCollapsed)}
                >
                  <ChevronDown className={sectionToggleIconClass(faqCollapsed)} />
                </button>
              </div>
              {!faqCollapsed && (
                <div className="flex flex-col gap-3 px-6 pb-6" id={sectionContentId('faq')}>
                  <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('faq_intro')}</p>
                  <div className="grid gap-3">
                    {[
                      { title: t('faq_step1_title'), body: t('faq_step1_body'), open: true },
                      { title: t('faq_step2_title'), body: t('faq_step2_body') },
                      { title: t('faq_step3_title'), body: t('faq_step3_body') },
                      { title: t('faq_step4_title'), body: t('faq_step4_body') }
                    ].map((item) => (
                      <details
                        key={item.title}
                        open={item.open}
                        className={cn(
                          'group rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-4 py-3',
                          'transition duration-200 ease-out open:-translate-y-0.5 open:shadow-[var(--shadow-1)]'
                        )}
                      >
                        <summary
                          className={cn(
                            'flex cursor-pointer list-none items-center justify-between font-semibold text-[var(--md-sys-color-on-surface)]',
                            "after:content-['+'] after:text-lg after:text-[var(--md-sys-color-primary)]",
                            'group-open:after:rotate-45 group-open:after:transition-transform'
                          )}
                        >
                          {item.title}
                        </summary>
                        <p className="mt-2 text-sm text-[var(--md-sys-color-on-surface-variant)]">{item.body}</p>
                      </details>
                    ))}
                  </div>
                </div>
              )}
            </section>

            <section
              id="recording"
              className={cn(
                'reveal flex flex-col gap-3 rounded-[30px] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container-low)] shadow-[var(--shadow-1)]',
                'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
                sectionHighlightClass('recording')
              )}
              style={delayStyle(500)}
            >
              <div className="flex flex-wrap items-center gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                <Video className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                <div className="flex min-w-0 flex-1 flex-wrap items-center justify-between gap-3">
                  <h2 className="font-display text-lg font-semibold">{t('section_recording')}</h2>
                  <span
                    className={cn(
                      'inline-flex items-center gap-2 rounded-full px-3 py-1 text-xs font-semibold',
                      recordingStatus?.active
                        ? 'bg-[var(--md-sys-color-error-container)] text-[var(--md-sys-color-on-surface)]'
                        : 'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface-variant)]'
                    )}
                  >
                    <span
                      className={cn(
                        'h-2 w-2 rounded-full',
                        recordingStatus?.active
                          ? 'bg-[var(--md-sys-color-error)]'
                          : 'bg-[var(--md-sys-color-outline)]'
                      )}
                    />
                    {recordingStatus?.active ? t('recording_status_active') : t('recording_status_idle')}
                  </span>
                </div>
                <button
                  className={sectionToggleClassName}
                  type="button"
                  onClick={() => toggleSection('recording')}
                  aria-expanded={!recordingCollapsed}
                  aria-controls={sectionContentId('recording')}
                  aria-label={sectionToggleLabel(recordingCollapsed)}
                  title={sectionToggleLabel(recordingCollapsed)}
                >
                  <ChevronDown className={sectionToggleIconClass(recordingCollapsed)} />
                </button>
              </div>
              {!recordingCollapsed && (
                <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('recording')}>
                  <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('recording_note')}</p>
                  <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('recording_format_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingFormat}
                        onChange={(event) => updateForm('recordingFormat', event.target.value)}
                      >
                        <option value="mp4">MP4</option>
                        <option value="mkv">MKV</option>
                      </select>
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="inline-flex items-center gap-2 text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        <Mic className="h-3.5 w-3.5" />
                        {t('recording_audio_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingAudioSource}
                        onChange={(event) => updateForm('recordingAudioSource', event.target.value)}
                      >
                        <option value="output">{t('recording_audio_device')}</option>
                        <option value="mic">{t('recording_audio_mic')}</option>
                        <option value="none">{t('recording_audio_none')}</option>
                      </select>
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('recording_connection_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingConnection}
                        onChange={(event) => updateForm('recordingConnection', event.target.value)}
                      >
                        <option value="">{t('connection_any')}</option>
                        <option value="usb">{t('connection_usb')}</option>
                        <option value="wifi">{t('connection_wifi')}</option>
                      </select>
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('recording_device_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingDevice}
                        onChange={(event) => updateForm('recordingDevice', event.target.value)}
                      >
                        <option value="">{t('recording_device_auto')}</option>
                        {scrcpyOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(220px,1fr))]">
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('recording_file_prefix_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingFilePrefix}
                        onChange={(event) => updateForm('recordingFilePrefix', event.target.value)}
                        placeholder={t('recording_file_prefix_placeholder')}
                      />
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('recording_output_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.recordingOutputDir}
                        onChange={(event) => updateForm('recordingOutputDir', event.target.value)}
                        placeholder={t('recording_output_placeholder')}
                      />
                    </div>
                  </div>

                  <div className="flex flex-wrap items-center gap-2">
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] px-4 py-2 text-sm font-semibold',
                        'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      type="button"
                      onClick={() => void selectRecordingDir()}
                    >
                      <FolderOpen className="h-4 w-4" />
                      {t('recording_output_choose')}
                    </button>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                        'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      type="button"
                      onClick={() => void saveRecordingDefaults()}
                      disabled={recordingSaving}
                    >
                      <Save className="h-4 w-4" />
                      {t('recording_output_save')}
                    </button>
                  </div>

                  <div className="rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-4 py-3">
                    <span className="text-xs font-semibold text-[var(--md-sys-color-on-surface-variant)]">
                      {t('recording_preview_label')}
                    </span>
                    <p className="mt-1 truncate text-xs text-[var(--md-sys-color-on-surface)]">
                      {recordingPreviewPath}
                    </p>
                  </div>

                  <div className="grid gap-3 rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                    {[
                      { key: 'recordingShowPreview', label: t('recording_show_preview') },
                      { key: 'recordingStayAwake', label: t('recording_stay_awake') },
                      { key: 'recordingShowTouches', label: t('recording_show_touches') },
                      { key: 'recordingTurnScreenOff', label: t('recording_turn_screen_off') }
                    ].map((item) => (
                      <label
                        key={item.key}
                        className="flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-surface-container-low)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition hover:border-[var(--md-sys-color-outline)]"
                      >
                        <input
                          type="checkbox"
                          className="h-4 w-4 accent-[var(--md-sys-color-primary)]"
                          checked={form[item.key as keyof FormState] as boolean}
                          onChange={(event) => updateForm(item.key as keyof FormState, event.target.checked)}
                        />
                        <span>{item.label}</span>
                      </label>
                    ))}
                  </div>

                  <p className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                    {t('recording_quality_note')}
                  </p>

                  <div className="flex flex-wrap gap-3">
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-5 py-2.5 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                        'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void startRecording()}
                      disabled={recordingStatus?.active || recordingLoading}
                    >
                      <Video className="h-4 w-4" />
                      {t('recording_start_button')}
                    </button>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error)] px-5 py-2.5 text-sm font-semibold text-white',
                        'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(180,35,24,0.35)]'
                      )}
                      onClick={() => void stopRecording()}
                      disabled={!recordingStatus?.active || recordingLoading}
                    >
                      <StopCircle className="h-4 w-4" />
                      {t('recording_stop_button')}
                    </button>
                  </div>
                </div>
              )}
            </section>

            <section className="grid gap-6 [grid-template-columns:repeat(auto-fit,minmax(280px,1fr))]">
              <article
                id="active-devices"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('active-devices')
                )}
                style={delayStyle(140)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Activity className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_active_devices')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('active-devices')}
                    aria-expanded={!activeDevicesCollapsed}
                    aria-controls={sectionContentId('active-devices')}
                    aria-label={sectionToggleLabel(activeDevicesCollapsed)}
                    title={sectionToggleLabel(activeDevicesCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(activeDevicesCollapsed)} />
                  </button>
                </div>
                {!activeDevicesCollapsed && (
                  <div className="flex flex-1 flex-col gap-3 px-6 pb-6" id={sectionContentId('active-devices')}>
                    {devicesLoading ? (
                      <div className="loading-spinner" />
                    ) : activeDevices.length === 0 ? (
                      <div className="grid place-items-center gap-2 rounded-[var(--radius-md)] border border-dashed border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-6 text-center text-sm text-[var(--md-sys-color-on-surface-variant)]">
                        <PhoneOff className="h-12 w-12 opacity-30" />
                        <p>{t('active_empty')}</p>
                      </div>
                    ) : (
                      <div className="flex flex-col gap-3">
                        {activeDevices.map((device) => {
                          const isWifi = device.serial.includes(':');
                          return (
                            <div
                              key={device.serial}
                              className={cn(
                                'flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)]',
                                'bg-[var(--md-sys-color-surface-container-low)] px-4 py-3 shadow-[var(--shadow-1)] transition duration-200 ease-out',
                                'hover:-translate-y-0.5 hover:shadow-[var(--shadow-1)]'
                              )}
                            >
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2 font-semibold">
                                  {isWifi ? (
                                    <Wifi className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                                  ) : (
                                    <Usb className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                                  )}
                                  <span className="break-words">{device.serial}</span>
                                </div>
                                <div className="text-xs text-[var(--md-sys-color-on-surface-variant)]">{device.status}</div>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                {isWifi && (
                                  <button
                                    className={cn(
                                      'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-3 py-1.5 text-xs font-semibold',
                                      'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                      'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                                    )}
                                    onClick={() => fillSaveDeviceForm(device.serial)}
                                  >
                                    <Save className="h-4 w-4" />
                                    {t('device_save')}
                                  </button>
                                )}
                                <button
                                  className={cn(
                                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error)] px-3 py-1.5 text-xs font-semibold text-white',
                                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(180,35,24,0.35)]'
                                  )}
                                  onClick={() => void disconnectDevice(device.serial)}
                                >
                                  <PowerOff className="h-4 w-4" />
                                  {t('device_disconnect')}
                                </button>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                )}
              </article>
              <article
                id="saved-devices"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('saved-devices')
                )}
                style={delayStyle(200)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Save className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_saved_devices')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('saved-devices')}
                    aria-expanded={!savedDevicesCollapsed}
                    aria-controls={sectionContentId('saved-devices')}
                    aria-label={sectionToggleLabel(savedDevicesCollapsed)}
                    title={sectionToggleLabel(savedDevicesCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(savedDevicesCollapsed)} />
                  </button>
                </div>
                {!savedDevicesCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('saved-devices')}>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('device_save_name_label')}
                      </label>
                      <input
                        ref={saveNameRef}
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.saveDeviceName}
                        onChange={(event) => updateForm('saveDeviceName', event.target.value)}
                        placeholder={t('device_save_name_placeholder')}
                      />
                    </div>
                    <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                      <div className="flex flex-col gap-2">
                        <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                          {t('device_save_ip_label')}
                        </label>
                        <input
                          className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                          value={form.saveDeviceIp}
                          onChange={(event) => updateForm('saveDeviceIp', event.target.value)}
                          placeholder="192.168.1.100"
                        />
                      </div>
                      <div className="flex flex-col gap-2">
                        <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                          {t('device_save_port_label')}
                        </label>
                        <input
                          className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                          value={form.saveDevicePort}
                          onChange={(event) => updateForm('saveDevicePort', event.target.value)}
                        />
                      </div>
                    </div>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                        'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void saveDeviceFromForm()}
                    >
                      <Save className="h-4 w-4" />
                      {t('device_save_button')}
                    </button>
                    <div>
                      {savedLoading ? (
                        <div className="loading-spinner" />
                      ) : savedDevices.length === 0 ? (
                        <div className="grid place-items-center gap-2 rounded-[var(--radius-md)] border border-dashed border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-6 text-center text-sm text-[var(--md-sys-color-on-surface-variant)]">
                          <SaveOff className="h-12 w-12 opacity-30" />
                          <p>{t('saved_empty')}</p>
                        </div>
                      ) : (
                        <div className="flex flex-col gap-3">
                          {savedDevices.map((device) => (
                            <div
                              key={`${device.ip}:${device.port}`}
                              className={cn(
                                'flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)]',
                                'bg-[var(--md-sys-color-surface-container-low)] px-4 py-3 transition duration-200 ease-out'
                              )}
                            >
                              <div className="min-w-0 flex-1">
                                <div className="flex items-center gap-2 font-semibold">
                                  {device.connection_type === 'wifi' ? (
                                    <Wifi className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                                  ) : (
                                    <Usb className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                                  )}
                                  <span className="break-words">{device.name}</span>
                                </div>
                                <div className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                                  {device.ip}:{device.port}
                                </div>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                <button
                                  className={cn(
                                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-3 py-1.5 text-xs font-semibold text-[var(--md-sys-color-on-primary)]',
                                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                                  )}
                                  onClick={() => void connectSaved(`${device.ip}:${device.port}`)}
                                >
                                  <Zap className="h-4 w-4" />
                                  {t('device_connect')}
                                </button>
                                <button
                                  className={cn(
                                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error)] px-3 py-1.5 text-xs font-semibold text-white',
                                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(180,35,24,0.35)]'
                                  )}
                                  onClick={() => void deleteSaved(device.ip, device.port)}
                                >
                                  <Trash2 className="h-4 w-4" />
                                  {t('device_delete')}
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </article>
              <article
                id="quick-connect"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('quick-connect')
                )}
                style={delayStyle(260)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <PlugZap className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_quick_connect')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('quick-connect')}
                    aria-expanded={!quickConnectCollapsed}
                    aria-controls={sectionContentId('quick-connect')}
                    aria-label={sectionToggleLabel(quickConnectCollapsed)}
                    title={sectionToggleLabel(quickConnectCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(quickConnectCollapsed)} />
                  </button>
                </div>
                {!quickConnectCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('quick-connect')}>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('quick_connect_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.quickConnectAddress}
                        onChange={(event) => updateForm('quickConnectAddress', event.target.value)}
                        placeholder={t('quick_connect_placeholder')}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter') {
                            void quickConnect();
                          }
                        }}
                      />
                    </div>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                        'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void quickConnect()}
                    >
                      <PlugZap className="h-4 w-4" />
                      {t('quick_connect_button')}
                    </button>
                  </div>
                )}
              </article>

              <article
                id="pairing"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('pairing')
                )}
                style={delayStyle(320)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Wifi className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_pairing')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('pairing')}
                    aria-expanded={!pairingCollapsed}
                    aria-controls={sectionContentId('pairing')}
                    aria-label={sectionToggleLabel(pairingCollapsed)}
                    title={sectionToggleLabel(pairingCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(pairingCollapsed)} />
                  </button>
                </div>
                {!pairingCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('pairing')}>
                    <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('pairing_note')}</p>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('pair_address_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.pairAddress}
                        onChange={(event) => updateForm('pairAddress', event.target.value)}
                        placeholder="192.168.1.100:12345"
                      />
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('pair_code_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.pairCode}
                        onChange={(event) => updateForm('pairCode', event.target.value)}
                        placeholder="123456"
                        maxLength={6}
                      />
                    </div>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                        'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void pairDevice()}
                    >
                      <Key className="h-4 w-4" />
                      {t('pair_button')}
                    </button>
                  </div>
                )}
              </article>

              <article
                id="usb-wifi"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('usb-wifi')
                )}
                style={delayStyle(380)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Power className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_usb_wifi')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('usb-wifi')}
                    aria-expanded={!usbWifiCollapsed}
                    aria-controls={sectionContentId('usb-wifi')}
                    aria-label={sectionToggleLabel(usbWifiCollapsed)}
                    title={sectionToggleLabel(usbWifiCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(usbWifiCollapsed)} />
                  </button>
                </div>
                {!usbWifiCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('usb-wifi')}>
                    <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('usb_wifi_note')}</p>
                    <div className="rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-4 py-3">
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="flex flex-col gap-1">
                          <span className="text-sm font-semibold text-[var(--md-sys-color-on-surface)]">
                            {t('connection_auto_label') || 'Auto-select connection'}
                          </span>
                          <span className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                            {t('connection_auto_hint') || 'Prefer Wi-Fi when latency is lower and stable.'}
                          </span>
                        </div>
                        <label className="inline-flex items-center gap-2 text-xs font-semibold text-[var(--md-sys-color-on-surface)]">
                          <input
                            type="checkbox"
                            className="h-4 w-4 accent-[var(--md-sys-color-primary)]"
                            checked={autoConnectionEnabled}
                            onChange={(event) => onAutoConnectionChange(event.target.checked)}
                            disabled={autoConnectionSaving}
                          />
                          {autoConnectionEnabled
                            ? t('connection_auto_on') || 'Enabled'
                            : t('connection_auto_off') || 'Disabled'}
                        </label>
                      </div>
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('usb_wifi_port_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.tcpipPort}
                        onChange={(event) => updateForm('tcpipPort', event.target.value)}
                      />
                    </div>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                        'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void enableTcpip()}
                    >
                      <Power className="h-4 w-4" />
                      {t('usb_wifi_button')}
                    </button>
                  </div>
                )}
              </article>
            </section>

            <section
              id="scrcpy"
              className={cn(
                'reveal flex flex-col gap-3 rounded-[30px] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container-low)] shadow-[var(--shadow-1)]',
                'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
                sectionHighlightClass('scrcpy')
              )}
              style={delayStyle(440)}
            >
              <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                <div className="flex items-center gap-3">
                  <Monitor className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                  <h2 className="font-display text-lg font-semibold">{t('section_scrcpy')}</h2>
                </div>
                <button
                  className={sectionToggleClassName}
                  type="button"
                  onClick={() => toggleSection('scrcpy')}
                  aria-expanded={!scrcpyCollapsed}
                  aria-controls={sectionContentId('scrcpy')}
                  aria-label={sectionToggleLabel(scrcpyCollapsed)}
                  title={sectionToggleLabel(scrcpyCollapsed)}
                >
                  <ChevronDown className={sectionToggleIconClass(scrcpyCollapsed)} />
                </button>
              </div>
              {!scrcpyCollapsed && (
                <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('scrcpy')}>
                  <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_preset_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.scrcpyPreset}
                        onChange={(event) => {
                          const value = event.target.value;
                          if (value !== 'custom') {
                            applyPreset(value);
                          } else {
                            updateForm('scrcpyPreset', 'custom');
                          }
                        }}
                      >
                        <option value="custom">{t('scrcpy_preset_custom')}</option>
                        {presets.map((preset) => (
                          <option key={preset.name} value={preset.name}>
                            {preset.name} ({preset.bitrate}/{preset.maxsize}px)
                          </option>
                        ))}
                      </select>
                    </div>

                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_keyboard_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.keyboard}
                        onChange={(event) => updateForm('keyboard', event.target.value)}
                      >
                        <option value="uhid">UHID</option>
                        <option value="sdk">SDK</option>
                        <option value="aoa">AOA</option>
                      </select>
                    </div>

                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_connection_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.connection}
                        onChange={(event) => updateForm('connection', event.target.value)}
                      >
                        <option value="">{t('connection_any')}</option>
                        <option value="usb">{t('connection_usb')}</option>
                        <option value="wifi">{t('connection_wifi')}</option>
                      </select>
                    </div>

                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_device_label')}
                      </label>
                      <select
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.scrcpyDevice}
                        onChange={(event) => updateForm('scrcpyDevice', event.target.value)}
                      >
                        <option value="">{t('scrcpy_device_auto')}</option>
                        {scrcpyOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                  <div className="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_bitrate_label')}
                      </label>
                      <input
                        type="number"
                        min={1}
                        max={100}
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.bitrateNum}
                        onChange={(event) => {
                          const value = event.target.value;
                          setForm((prev) => ({ ...prev, bitrateNum: value, scrcpyPreset: 'custom' }));
                        }}
                      />
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('scrcpy_maxsize_label')}
                      </label>
                      <input
                        type="number"
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.maxsize}
                        onChange={(event) => {
                          const value = event.target.value;
                          setForm((prev) => ({ ...prev, maxsize: value, scrcpyPreset: 'custom' }));
                        }}
                      />
                    </div>
                  </div>

                  <div className="grid gap-3 rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-4 [grid-template-columns:repeat(auto-fit,minmax(200px,1fr))]">
                    {[
                      { key: 'stayAwake', label: t('scrcpy_stay_awake') },
                      { key: 'showTouches', label: t('scrcpy_show_touches') },
                      { key: 'turnScreenOff', label: t('scrcpy_turn_screen_off') },
                      { key: 'fullscreen', label: t('scrcpy_fullscreen') },
                      { key: 'noAudio', label: t('scrcpy_no_audio') }
                    ].map((item) => (
                      <label
                        key={item.key}
                        className="flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-surface-container-low)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition hover:border-[var(--md-sys-color-outline)]"
                      >
                        <input
                          type="checkbox"
                          className="h-4 w-4 accent-[var(--md-sys-color-primary)]"
                          checked={form[item.key as keyof FormState] as boolean}
                          onChange={(event) => updateForm(item.key as keyof FormState, event.target.checked)}
                        />
                        <span>{item.label}</span>
                      </label>
                    ))}
                  </div>

                  <button
                    className={cn(
                      'inline-flex w-full items-center justify-center gap-2 rounded-full bg-[var(--button-success-bg)] px-6 py-3 text-base font-semibold text-white',
                      'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                      'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                    )}
                    onClick={() => void launchScrcpy()}
                  >
                    <Play className="h-5 w-5" />
                    {t('scrcpy_launch_button')}
                  </button>
                </div>
              )}
            </section>

            <section className="grid gap-6 [grid-template-columns:repeat(auto-fit,minmax(280px,1fr))]">
              <article
                id="presets"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('presets')
                )}
                style={delayStyle(500)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Bookmark className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_presets')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('presets')}
                    aria-expanded={!presetsCollapsed}
                    aria-controls={sectionContentId('presets')}
                    aria-label={sectionToggleLabel(presetsCollapsed)}
                    title={sectionToggleLabel(presetsCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(presetsCollapsed)} />
                  </button>
                </div>
                {!presetsCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('presets')}>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('preset_name_label')}
                      </label>
                      <input
                        className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:bg-[var(--md-sys-color-surface)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                        value={form.presetName}
                        onChange={(event) => updateForm('presetName', event.target.value)}
                        placeholder={t('preset_name_placeholder')}
                      />
                    </div>
                    <button
                      className={cn(
                        'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                        'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                        'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                      )}
                      onClick={() => void savePreset()}
                    >
                      <Save className="h-4 w-4" />
                      {t('preset_save_button')}
                    </button>
                    <div className="flex flex-col gap-3">
                      <h3 className="font-display text-sm font-semibold">{t('preset_list_title')}</h3>
                      {presets.length === 0 ? (
                        <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('preset_empty')}</p>
                      ) : (
                        <div className="flex flex-col gap-3">
                          {presets.map((preset) => (
                            <div
                              key={preset.name}
                              className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container-low)] px-4 py-3"
                            >
                              <div className="flex min-w-0 flex-1 flex-col gap-1">
                                <strong className="text-sm">{preset.name}</strong>
                                <span className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                                  {preset.bitrate} / {preset.maxsize}px
                                </span>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                <button
                                  className={cn(
                                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-3 py-1.5 text-xs font-semibold text-[var(--md-sys-color-on-primary)]',
                                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                                  )}
                                  onClick={() => applyPreset(preset.name)}
                                >
                                  {t('preset_apply_button')}
                                </button>
                                <button
                                  className={cn(
                                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error)] px-3 py-1.5 text-xs font-semibold text-white',
                                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                                    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(180,35,24,0.35)]'
                                  )}
                                  onClick={() => void removePreset(preset.name)}
                                >
                                  {t('preset_delete_button')}
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </article>
              <article
                id="diagnostics"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('diagnostics')
                )}
                style={delayStyle(560)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-5 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <ShieldCheck className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_diagnostics')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('diagnostics')}
                    aria-expanded={!diagnosticsCollapsed}
                    aria-controls={sectionContentId('diagnostics')}
                    aria-label={sectionToggleLabel(diagnosticsCollapsed)}
                    title={sectionToggleLabel(diagnosticsCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(diagnosticsCollapsed)} />
                  </button>
                </div>
                {!diagnosticsCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('diagnostics')}>
                    <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('diagnostics_note')}</p>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('downloads_base_dir_label')}
                      </label>
                      <div className="flex flex-wrap items-center gap-2">
                        <input
                          className="min-w-[220px] flex-1 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                          value={downloadsBaseDir}
                          onChange={(event) => setDownloadsBaseDir(event.target.value)}
                          placeholder={t('downloads_base_dir_placeholder')}
                        />
                        <button
                          className={cn(
                            'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                            'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                          )}
                          type="button"
                          onClick={() => void saveDownloadsBaseDir(downloadsBaseDir.trim())}
                          disabled={!downloadsBaseDir.trim()}
                        >
                          <Save className="h-4 w-4" />
                          {t('downloads_base_dir_save')}
                        </button>
                        <button
                          className={cn(
                            'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] px-4 py-2 text-sm font-semibold',
                            'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                          )}
                          type="button"
                          onClick={() => void selectDownloadsBaseDir()}
                        >
                          <FolderOpen className="h-4 w-4" />
                          {t('downloads_base_dir_choose')}
                        </button>
                      </div>
                      <span className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                        {t('downloads_base_dir_note')}
                      </span>
                    </div>
                    <div className="flex flex-col gap-2">
                      <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                        {t('logs_export_dir_label')}
                      </label>
                      <div className="flex flex-wrap items-center gap-2">
                        <input
                          className="min-w-[220px] flex-1 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                          value={logsExportDir}
                          onChange={(event) => setLogsExportDir(event.target.value)}
                          placeholder={t('logs_export_dir_placeholder')}
                        />
                        <button
                          className={cn(
                            'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                            'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                          )}
                          type="button"
                          onClick={() => void saveLogsExportDir(logsExportDir.trim())}
                          disabled={!logsExportDir.trim()}
                        >
                          <Save className="h-4 w-4" />
                          {t('logs_export_dir_save')}
                        </button>
                        <button
                          className={cn(
                            'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] px-4 py-2 text-sm font-semibold',
                            'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                          )}
                          type="button"
                          onClick={() => void selectLogsExportDir()}
                        >
                          <FolderOpen className="h-4 w-4" />
                          {t('logs_export_dir_choose')}
                        </button>
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-3">
                      <button
                        className={cn(
                          'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                          'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                          'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                        )}
                        onClick={() => void exportLogs()}
                      >
                        <Download className="h-4 w-4" />
                        {t('logs_export_button')}
                      </button>
                      <button
                        className={cn(
                          'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                          'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                          'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                        )}
                        onClick={() => void checkUpdates()}
                      >
                        <RefreshCw className="h-4 w-4" />
                        {t('update_check_button')}
                      </button>
                    </div>
                  </div>
                )}
              </article>
              <article
                id="config"
                className={cn(
                  'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
                  'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)] focus-within:-translate-y-0.5 focus-within:shadow-[var(--shadow-2)] focus-within:border-[var(--accent-border-strong)]',
                  sectionHighlightClass('config')
                )}
                style={delayStyle(620)}
              >
                <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-5 text-[var(--md-sys-color-primary)]">
                  <div className="flex items-center gap-3">
                    <Key className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
                    <h2 className="font-display text-lg font-semibold">{t('section_config')}</h2>
                  </div>
                  <button
                    className={sectionToggleClassName}
                    type="button"
                    onClick={() => toggleSection('config')}
                    aria-expanded={!configCollapsed}
                    aria-controls={sectionContentId('config')}
                    aria-label={sectionToggleLabel(configCollapsed)}
                    title={sectionToggleLabel(configCollapsed)}
                  >
                    <ChevronDown className={sectionToggleIconClass(configCollapsed)} />
                  </button>
                </div>
                {!configCollapsed && (
                  <div className="flex flex-col gap-4 px-6 pb-6" id={sectionContentId('config')}>
                    <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('config_note')}</p>
                    <textarea
                      className="min-h-[220px] w-full rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-3 font-mono text-xs text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                      value={configDraft}
                      onChange={(event) => setConfigDraft(event.target.value)}
                      placeholder="{ }"
                      spellCheck={false}
                    />
                    <div className="flex flex-wrap gap-3">
                      <button
                        className={cn(
                          'inline-flex items-center gap-2 rounded-full border border-transparent bg-[var(--md-sys-color-secondary-container)] px-4 py-2 text-sm font-semibold',
                          'text-[var(--button-secondary-text)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                          'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                        )}
                        type="button"
                        onClick={() => void loadFullConfig()}
                        disabled={configLoading}
                      >
                        <RefreshCw className="h-4 w-4" />
                        {t('config_reload_button')}
                      </button>
                      <button
                        className={cn(
                          'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                          'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                          'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
                        )}
                        type="button"
                        onClick={() => void saveFullConfig()}
                        disabled={configSaving || !configDraft.trim()}
                      >
                        <Save className="h-4 w-4" />
                        {t('config_save_button')}
                      </button>
                    </div>
                  </div>
                )}
              </article>

            </section>
    </>
  );
}
