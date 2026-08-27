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
  serviceCommands: string[];
};

// Fallback only: the real list comes from GET /api/service/commands so the
// two sides cannot drift into 404s.
const FALLBACK_COMMANDS = [
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
];

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
  serviceOutput,
  serviceCommands
}: ServiceMenuPageProps) {
  const commands = serviceCommands.length ? serviceCommands : FALLBACK_COMMANDS;
  const sectionHighlightClass = (sectionId: string) =>
    activeSection === sectionId ? 'm3-panel--active' : '';
  const sectionToggleClassName = 'm3-icon-btn m3-state m3-icon-btn--sm m3-icon-btn--outlined';
  const sectionToggleIconClass = (collapsed: boolean) =>
    cn('m3-panel__toggle-icon', collapsed && 'm3-panel__toggle-icon--collapsed');

  return (
    <section className="flex flex-col gap-6">
      <article
        id="service-menu"
        className={cn(
          'm3-enter m3-panel',
          sectionHighlightClass('service-menu')
        )}
        style={delayStyle(160)}
      >
        <div className="m3-panel__header">
          <div className="flex items-center gap-3">
            <Activity className="m3-token" />
            <h2 className="m3-panel__title">
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
          <div className="m3-panel__body">
            <p className="m3-body-medium m3-on-variant">
              {t('service_quick_title')}
            </p>
            <div className="flex flex-wrap gap-2">
              {commands.map((cmd) => (
                <button
                  key={cmd}
                  className={cn(
                    'm3-btn m3-state m3-btn--elevated m3-btn--xs',
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
              <label className="m3-field-label">
                {t('service_custom_label')}
              </label>
              <div className="flex gap-2">
                <input
                  className="m3-field flex-1"
                  placeholder="ls -la /sdcard"
                  value={serviceCommand}
                  onChange={(e) => setServiceCommand(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && void runCustomCommand()}
                />
                <button
                  className="m3-btn m3-state m3-btn--filled m3-btn--sm"
                  type="button"
                  onClick={() => void runCustomCommand()}
                  disabled={serviceLoading || !serviceCommand.trim()}
                >
                  {t('service_run')}
                </button>
              </div>
            </div>
            {serviceOutput && (
              <pre className="m3-code">
                {serviceOutput}
              </pre>
            )}
          </div>
        )}
      </article>
    </section>
  );
}
