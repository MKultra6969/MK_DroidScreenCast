import { Lock, Sparkles } from 'lucide-react';
import { cn } from '../../utils';
import { delayStyle } from '../../lib/style';

type AutomationPageProps = {
  t: (key: string) => string;
  activeSection: string;
};

const WIP_SECTIONS = [
  { id: 'macros', titleKey: 'section_macros', fallback: 'Macros', delay: 160 },
  { id: 'scripts', titleKey: 'section_scripts', fallback: 'Scripts', delay: 200 }
];

export function AutomationPage({ t, activeSection }: AutomationPageProps) {
  const sectionHighlightClass = (sectionId: string) =>
    activeSection === sectionId
      ? 'ring-2 ring-[var(--md-sys-color-primary)] ring-offset-2 ring-offset-[var(--md-sys-color-background)]'
      : '';

  const wipBadge = t('automation_wip_badge');
  const wipBody = t('automation_wip_body');
  const wipNote = t('automation_wip_note');

  return (
    <section className="flex flex-col gap-6">
      {WIP_SECTIONS.map((section) => (
        <article
          key={section.id}
          id={section.id}
          className={cn(
            'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
            'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
            sectionHighlightClass(section.id)
          )}
          style={delayStyle(section.delay)}
        >
          <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
            <div className="flex items-center gap-3">
              <Sparkles className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
              <h2 className="font-display text-lg font-semibold">
                {t(section.titleKey) || section.fallback}
              </h2>
            </div>
            <span className="inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-secondary-container)] px-3 py-1 text-xs font-semibold text-[var(--md-sys-color-on-secondary-container)]">
              <Lock className="h-4 w-4" />
              {wipBadge}
            </span>
          </div>
          <div className="flex flex-col gap-2 px-6 pb-6 text-sm text-[var(--md-sys-color-on-surface-variant)]">
            <p>{wipBody}</p>
            <p className="text-xs">{wipNote}</p>
          </div>
        </article>
      ))}
    </section>
  );
}
