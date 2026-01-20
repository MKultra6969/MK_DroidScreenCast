import { cn } from '../../utils';

type BootOverlayProps = {
  ready: boolean;
  timedOut: boolean;
  onRetry: () => void;
};

export function BootOverlay({ ready, timedOut, onRetry }: BootOverlayProps) {
  if (ready) return null;

  return (
    <div className="boot-overlay fixed inset-0 z-[1500] flex items-center justify-center p-6">
      <div className="flex w-full max-w-[420px] flex-col items-center gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 text-center shadow-[var(--shadow-3-soft)]">
        <div className="loading-spinner" />
        <h2 className="font-display text-lg font-semibold">Starting MK DroidScreenCast</h2>
        <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">
          {timedOut
            ? 'The background service is taking longer than expected. Make sure it is running.'
            : 'Connecting to the background service...'}
        </p>
        {timedOut && (
          <button
            className={cn(
              'mt-2 inline-flex items-center justify-center rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
              'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
              'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
            )}
            type="button"
            onClick={onRetry}
          >
            Retry
          </button>
        )}
      </div>
    </div>
  );
}
