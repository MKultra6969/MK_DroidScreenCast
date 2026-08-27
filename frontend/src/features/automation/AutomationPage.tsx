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
    activeSection === sectionId ? 'm3-panel--active' : '';

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
            'm3-enter m3-panel',
            sectionHighlightClass(section.id)
          )}
          style={delayStyle(section.delay)}
        >
          <div className="m3-panel__header">
            <div className="flex items-center gap-3">
              <Sparkles className="m3-token" />
              <h2 className="m3-panel__title">
                {t(section.titleKey) || section.fallback}
              </h2>
            </div>
            <span className="m3-badge m3-badge--secondary">
              <Lock className="h-4 w-4" />
              {wipBadge}
            </span>
          </div>
          <div className="m3-panel__body m3-body-medium m3-on-variant">
            <p>{wipBody}</p>
            <p className="text-xs">{wipNote}</p>
          </div>
        </article>
      ))}
    </section>
  );
}
