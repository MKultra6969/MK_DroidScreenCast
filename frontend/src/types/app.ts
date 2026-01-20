export type Device = {
  serial: string;
  status: string;
};

export type SavedDevice = {
  ip: string;
  port: string;
  name: string;
  connection_type?: string;
};

export type Preset = {
  name: string;
  bitrate: string;
  maxsize: string | number;
};

export type Notification = {
  id: number;
  type: 'success' | 'error';
  message: string;
};

export type RecordingStatus = {
  active: boolean;
  pid?: number;
  started_at?: string;
  output_path?: string;
  format?: string;
  audio_source?: string;
  show_preview?: boolean;
  serial?: string;
  connection?: string;
  last_error?: {
    exit_code?: number;
    output?: string;
    timestamp?: string;
  };
};

export type FormState = {
  saveDeviceName: string;
  saveDeviceIp: string;
  saveDevicePort: string;
  quickConnectAddress: string;
  pairAddress: string;
  pairCode: string;
  tcpipPort: string;
  bitrateNum: string;
  maxsize: string;
  keyboard: string;
  connection: string;
  scrcpyDevice: string;
  scrcpyPreset: string;
  stayAwake: boolean;
  showTouches: boolean;
  turnScreenOff: boolean;
  fullscreen: boolean;
  noAudio: boolean;
  presetName: string;
  recordingFormat: string;
  recordingAudioSource: string;
  recordingOutputDir: string;
  recordingFilePrefix: string;
  recordingShowPreview: boolean;
  recordingStayAwake: boolean;
  recordingShowTouches: boolean;
  recordingTurnScreenOff: boolean;
  recordingConnection: string;
  recordingDevice: string;
};

export type Screenshot = {
  id: string;
  filename: string;
  caption: string;
  created_at: string;
};

export type FileEntry = {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  permissions?: string;
  date?: string;
};
