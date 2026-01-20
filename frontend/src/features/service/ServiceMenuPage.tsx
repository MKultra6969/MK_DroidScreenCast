import { Activity, ChevronDown } from 'lucide-react';
import { cn } from '../../utils';
import { delayStyle } from '../../lib/style';

type ServiceMenuPageProps = {
  t: (key: string) => string;
  activeSection: string;
  isSectionCollapsed: (sectionId: string) => boolean;
  toggleSection: (sectionId: string) => void;
  serviceLoading: boolean;
  serviceCommand: string;
  setServiceCommand: (value: string) => void;
  runServiceCommand: (command: string) => void | Promise<void>;
  runCustomCommand: () => void | Promise<void>;
  serviceOutput: string;
};

export function ServiceMenuPage({
  t,
  activeSection,
  isSectionCollapsed,
  toggleSection,
  serviceLoading,
  serviceCommand,
  setServiceCommand,
  runServiceCommand,
  runCustomCommand,
  serviceOutput
}: ServiceMenuPageProps) {
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

  return (
    <section className="flex flex-col gap-6">
      <article
        id="service-menu"
        className={cn(
          'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
          'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
          sectionHighlightClass('service-menu')
        )}
        style={delayStyle(160)}
      >
        <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
          <div className="flex items-center gap-3">
            <Activity className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 className="font-display text-lg font-semibold">
              {t('section_service_menu') || 'Service Menu'}
            </h2>
          </div>
          <button
            className={sectionToggleClassName}
            type="button"
            onClick={() => toggleSection('service-menu')}
            aria-expanded={!isSectionCollapsed('service-menu')}
          >
            <ChevronDown className={sectionToggleIconClass(isSectionCollapsed('service-menu'))} />
          </button>
        </div>
        {!isSectionCollapsed('service-menu') && (
          <div className="flex flex-col gap-4 px-6 pb-6">
            <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">
              Quick ADB diagnostic commands
            </p>
            <div className="flex flex-wrap gap-2">
              {[
                'battery',
                'wifi',
                'memory',
                'disk',
                'screen',
                'processes',
                'cpu',
                'top',
                'props',
                'packages',
                'uptime',
                'network',
                'thermal'
              ].map((cmd) => (
                <button
                  key={cmd}
                  className={cn(
                    'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-1.5 text-xs font-semibold',
                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
                    serviceLoading && 'opacity-50 cursor-not-allowed'
                  )}
                  type="button"
                  onClick={() => void runServiceCommand(cmd)}
                  disabled={serviceLoading}
                >
                  {cmd}
                </button>
              ))}
            </div>
            <div className="flex flex-col gap-2">
              <label className="text-xs font-semibold tracking-wide text-[var(--md-sys-color-on-surface-variant)]">
                Custom command
              </label>
              <div className="flex gap-2">
                <input
                  className="flex-1 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-sm text-[var(--md-sys-color-on-surface)] transition focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
                  placeholder="ls -la /sdcard"
                  value={serviceCommand}
                  onChange={(e) => setServiceCommand(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && void runCustomCommand()}
                />
                <button
                  className={cn(
                    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]'
                  )}
                  type="button"
                  onClick={() => void runCustomCommand()}
                  disabled={serviceLoading || !serviceCommand.trim()}
                >
                  Run
                </button>
              </div>
            </div>
            {serviceOutput && (
              <pre className="max-h-64 overflow-auto rounded-[var(--radius-sm)] bg-[var(--md-sys-color-surface-container)] p-4 text-xs font-mono text-[var(--md-sys-color-on-surface)]">
                {serviceOutput}
              </pre>
            )}
          </div>
        )}
      </article>
    </section>
  );
}
