import { RefreshCw, Smartphone, TriangleAlert } from 'lucide-react';

type BootOverlayProps = {
  ready: boolean;
  timedOut: boolean;
  onRetry: () => void;
  progress?: string;
};

export function BootOverlay({ ready, timedOut, onRetry, progress }: BootOverlayProps) {
  if (ready) return null;

  return (
    <div className="m3-scrim z-[1500] flex items-center justify-center p-6">
      <div className="m3-dialog m3-dialog--enter max-w-[440px] items-center text-center">
        <span
          className={
            timedOut
              ? 'grid h-16 w-16 place-items-center rounded-m3lg bg-error-container text-on-error-container'
              : 'grid h-16 w-16 place-items-center rounded-m3lg bg-primary-container text-on-primary-container'
          }
        >
          {timedOut ? <TriangleAlert className="h-8 w-8" /> : <Smartphone className="h-8 w-8" />}
        </span>

        <div className="flex flex-col gap-2">
          <h2 className="m3-title-large">Starting MK DroidScreenCast</h2>
          <p className="m3-body-medium m3-on-variant">
            {timedOut
              ? 'The background service is taking longer than expected. Make sure it is running.'
              : 'Connecting to the background service…'}
          </p>
        </div>

        {/* Первый запуск качает platform-tools и scrcpy — показываем что
            именно происходит, а не безмолвный кружок. */}
        {!timedOut && (
          <div className="flex w-full flex-col gap-3">
            <div className="m3-linear" />
            {progress && <p className="m3-body-small m3-on-variant">{progress}</p>}
          </div>
        )}

        {timedOut && (
          <button className="m3-btn m3-state m3-btn--filled" type="button" onClick={onRetry}>
            <RefreshCw />
            Retry
          </button>
        )}
      </div>
    </div>
  );
}
