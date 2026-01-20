import { Check, Sparkles, X } from 'lucide-react';
import { cn } from '../../utils';

type FirstRunModalProps = {
  open: boolean;
  onClose: () => void;
  t: (key: string) => string;
};

export function FirstRunModal({ open, onClose, t }: FirstRunModalProps) {
  return (
    <div
      className={cn(
        'modal-overlay fixed inset-0 z-[1200] flex items-center justify-center p-5 transition-opacity',
        open ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'
      )}
      aria-hidden={!open}
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className="relative flex w-full max-w-[560px] flex-col gap-4 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 shadow-[var(--shadow-3)]"
        role="dialog"
        aria-modal="true"
        aria-labelledby="firstRunTitle"
      >
        <button
          className="absolute right-4 top-4 text-[var(--md-sys-color-on-surface-variant)]"
          onClick={onClose}
          aria-label="Close"
        >
          <X className="h-5 w-5" />
        </button>
        <div className="flex items-center gap-3 text-[var(--md-sys-color-primary)]">
          <Sparkles className="h-6 w-6 rounded-[14px] bg-[var(--md-sys-color-primary-container)] p-2 text-[var(--md-sys-color-on-primary-container)]" />
          <h2 id="firstRunTitle" className="font-display text-lg font-semibold">
            {t('first_run_title')}
          </h2>
        </div>
        <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('first_run_intro')}</p>
        <div className="grid gap-3">
          {[
            { title: t('faq_step1_title'), body: t('faq_step1_body') },
            { title: t('faq_step2_title'), body: t('faq_step2_body') },
            { title: t('faq_step3_title'), body: t('faq_step3_body') },
            { title: t('faq_step4_title'), body: t('faq_step4_body') }
          ].map((item) => (
            <div
              key={item.title}
              className="rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-4 py-3"
            >
              <h3 className="text-sm font-semibold">{item.title}</h3>
              <p className="mt-1 text-xs text-[var(--md-sys-color-on-surface-variant)]">{item.body}</p>
            </div>
          ))}
        </div>
        <button
          className={cn(
            'inline-flex items-center gap-2 self-start rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
            'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
            'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
          )}
          onClick={onClose}
        >
          <Check className="h-4 w-4" />
          {t('first_run_button')}
        </button>
      </div>
    </div>
  );
}
