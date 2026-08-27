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
        'm3-scrim z-[1200] flex items-center justify-center p-5',
        'transition-opacity duration-medium ease-emphasized',
        // invisible убирает элементы закрытого диалога из порядка обхода Tab
        open ? 'pointer-events-auto opacity-100' : 'pointer-events-none invisible opacity-0'
      )}
      aria-hidden={!open}
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className={cn('m3-dialog relative max-w-[580px]', open && 'm3-dialog--enter')}
        role="dialog"
        aria-modal="true"
        aria-labelledby="firstRunTitle"
      >
        <button
          className="m3-icon-btn m3-state m3-icon-btn--sm absolute right-4 top-4"
          onClick={onClose}
          aria-label="Close"
          type="button"
        >
          <X />
        </button>

        <div className="flex items-center gap-3.5 pr-10">
          <Sparkles className="m3-token" />
          <h2 id="firstRunTitle" className="m3-title-large">
            {t('first_run_title')}
          </h2>
        </div>

        <p className="m3-body-medium m3-on-variant">{t('first_run_intro')}</p>

        <ol className="flex flex-col gap-2.5">
          {[
            { title: t('faq_step1_title'), body: t('faq_step1_body') },
            { title: t('faq_step2_title'), body: t('faq_step2_body') },
            { title: t('faq_step3_title'), body: t('faq_step3_body') },
            { title: t('faq_step4_title'), body: t('faq_step4_body') }
          ].map((item, index) => (
            <li key={item.title} className="flex items-start gap-3 rounded-m3lg bg-surface-container p-4">
              <span className="m3-label-medium grid h-6 w-6 shrink-0 place-items-center rounded-full bg-primary text-on-primary">
                {index + 1}
              </span>
              <div className="min-w-0">
                <h3 className="m3-title-small">{item.title}</h3>
                <p className="m3-body-small m3-on-variant mt-1">{item.body}</p>
              </div>
            </li>
          ))}
        </ol>

        <button className="m3-btn m3-state m3-btn--filled self-start" onClick={onClose} type="button">
          <Check />
          {t('first_run_button')}
        </button>
      </div>
    </div>
  );
}
