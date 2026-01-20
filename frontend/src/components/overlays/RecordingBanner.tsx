import { StopCircle, Timer } from 'lucide-react';
import type { RecordingStatus } from '../../types/app';
import { cn } from '../../utils';

type RecordingBannerProps = {
  recordingStatus: RecordingStatus | null;
  recordingElapsed: string | null;
  recordingLoading: boolean;
  onStop: () => void;
  t: (key: string) => string;
};

export function RecordingBanner({
  recordingStatus,
  recordingElapsed,
  recordingLoading,
  onStop,
  t
}: RecordingBannerProps) {
  if (!recordingStatus?.active) return null;

  return (
    <div className="fixed left-1/2 top-6 z-[1300] w-[min(92vw,720px)] -translate-x-1/2">
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] px-4 py-3 shadow-[var(--shadow-2)]">
        <div className="flex min-w-0 items-center gap-3">
          <span className="h-2.5 w-2.5 rounded-full bg-[var(--md-sys-color-error)] shadow-[0_0_12px_rgba(180,35,24,0.5)]" />
          <div className="min-w-0">
            <p className="text-sm font-semibold">{t('recording_live_label')}</p>
            <p className="truncate text-xs text-[var(--md-sys-color-on-surface-variant)]">
              {recordingStatus?.output_path || t('recording_output_unknown')}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error-container)] px-3 py-1 text-xs font-semibold text-[var(--md-sys-color-on-surface)]">
            <Timer className="h-3.5 w-3.5" />
            <span>{recordingElapsed || '00:00'}</span>
          </div>
          <button
            className={cn(
              'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-error)] px-3 py-1.5 text-xs font-semibold text-white',
              'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
              'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgba(180,35,24,0.35)]'
            )}
            onClick={onStop}
            disabled={recordingLoading}
          >
            <StopCircle className="h-4 w-4" />
            {t('recording_stop_button')}
          </button>
        </div>
      </div>
    </div>
  );
}
