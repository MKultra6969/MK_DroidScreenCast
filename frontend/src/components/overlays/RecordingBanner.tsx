import { memo, useEffect, useState } from 'react';
import { CircleStop, Timer } from 'lucide-react';
import type { RecordingStatus } from '../../types/app';
import { formatDuration } from '../../lib/time';

type RecordingBannerProps = {
  recordingStatus: RecordingStatus | null;
  recordingLoading: boolean;
  onStop: () => void;
  t: (key: string) => string;
};

/**
 * Секундомер живёт здесь, а не в App.
 *
 * Раньше отметка времени тикала в состоянии корневого компонента, и раз в
 * секунду перерисовывалось всё дерево — вместе со списком файлов и галереей.
 * Теперь тик задевает только эту полоску.
 */
function useElapsed(active: boolean, startedAt?: string) {
  const [elapsed, setElapsed] = useState<string | null>(null);

  useEffect(() => {
    if (!active || !startedAt) {
      setElapsed(null);
      return;
    }

    const start = new Date(startedAt).getTime();
    if (Number.isNaN(start)) {
      setElapsed(null);
      return;
    }

    const tick = () => setElapsed(formatDuration(Date.now() - start));
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [active, startedAt]);

  return elapsed;
}

function RecordingBannerView({
  recordingStatus,
  recordingLoading,
  onStop,
  t
}: RecordingBannerProps) {
  const active = Boolean(recordingStatus?.active);
  const elapsed = useElapsed(active, recordingStatus?.started_at);

  if (!active) return null;

  return (
    <div className="pointer-events-none fixed left-1/2 top-5 z-[1300] w-[min(92vw,720px)] -translate-x-1/2">
      <div className="m3-dialog--enter pointer-events-auto flex flex-wrap items-center justify-between gap-3 rounded-m3xl bg-surface-high px-4 py-3 shadow-e3">
        <div className="flex min-w-0 items-center gap-3">
          <span className="m3-dot m3-dot--live" />
          <div className="min-w-0">
            <p className="m3-title-small">{t('recording_live_label')}</p>
            <p className="m3-body-small m3-on-variant truncate">
              {recordingStatus?.output_path || t('recording_output_unknown')}
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <span className="m3-badge m3-badge--error tabular-nums">
            <Timer />
            {elapsed || '00:00'}
          </span>
          <button
            className="m3-btn m3-state m3-btn--danger m3-btn--sm"
            onClick={onStop}
            disabled={recordingLoading}
            type="button"
          >
            <CircleStop />
            {t('recording_stop_button')}
          </button>
        </div>
      </div>
    </div>
  );
}

export const RecordingBanner = memo(RecordingBannerView);
