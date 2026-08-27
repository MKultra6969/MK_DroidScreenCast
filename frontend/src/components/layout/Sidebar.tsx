import { memo, useMemo } from 'react';
import type { ComponentType } from 'react';
import {
  ChevronRight,
  Folder,
  Home,
  Menu,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Smartphone,
  Sun,
  Wrench,
  X,
  Zap
} from 'lucide-react';
import { cn } from '../../utils';

type SidebarProps = {
  t: (key: string) => string;
  activeSection: string;
  sidebarOpen: boolean;
  sidebarMobileOpen: boolean;
  onToggleSidebar: () => void;
  onOpenMobile: () => void;
  onCloseMobile: () => void;
  expandedMenuGroups: Record<string, boolean>;
  onToggleMenuGroup: (groupId: string) => void;
  onNavigate: (sectionId: string) => void;
  wsConnected: boolean;
  theme: 'light' | 'dark';
  themePreference: 'auto' | 'light' | 'dark';
  onThemePreferenceChange: (value: 'auto' | 'light' | 'dark') => void;
  onToggleTheme: () => void;
  themeLabel: string;
};

type MenuChild = { id: string; label: string };

type MenuItem = {
  id: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
  /** Куда ведёт пункт, если у него нет детей. */
  target?: string;
  children?: MenuChild[];
};

const THEME_OPTIONS: Array<{
  value: 'auto' | 'light' | 'dark';
  icon: ComponentType<{ className?: string }>;
  labelKey: string;
  fallback: string;
}> = [
  { value: 'auto', icon: Monitor, labelKey: 'theme_system', fallback: 'System' },
  { value: 'light', icon: Sun, labelKey: 'theme_light', fallback: 'Light' },
  { value: 'dark', icon: Moon, labelKey: 'theme_dark', fallback: 'Dark' }
];

function SidebarView({
  t,
  activeSection,
  sidebarOpen,
  sidebarMobileOpen,
  onToggleSidebar,
  onOpenMobile,
  onCloseMobile,
  expandedMenuGroups,
  onToggleMenuGroup,
  onNavigate,
  wsConnected,
  theme,
  themePreference,
  onThemePreferenceChange,
  onToggleTheme,
  themeLabel
}: SidebarProps) {
  const menuItems: MenuItem[] = useMemo(
    () => [
      { id: 'home', icon: Home, label: t('menu_home') || 'Home', target: 'faq' },
      {
        id: 'devices',
        icon: Smartphone,
        label: t('menu_devices') || 'Devices',
        children: [
          { id: 'active-devices', label: t('section_active_devices') || 'Active' },
          { id: 'saved-devices', label: t('section_saved_devices') || 'Saved' },
          { id: 'quick-connect', label: t('section_quick_connect') || 'Quick Connect' },
          { id: 'pairing', label: t('section_pairing') || 'Pairing' },
          { id: 'usb-wifi', label: t('section_usb_wifi') || 'USB to WiFi' }
        ]
      },
      {
        id: 'tools',
        icon: Wrench,
        label: t('menu_tools') || 'Tools',
        children: [
          { id: 'scrcpy', label: t('section_scrcpy') || 'Scrcpy' },
          { id: 'recording', label: t('section_recording') || 'Recording' },
          { id: 'service-menu', label: t('section_service_menu') || 'Service Menu' }
        ]
      },
      {
        id: 'files',
        icon: Folder,
        label: t('menu_files') || 'Files',
        children: [
          { id: 'file-manager', label: t('section_file_manager') || 'File Manager' },
          { id: 'gallery', label: t('section_gallery') || 'Screenshots' }
        ]
      },
      {
        id: 'automation',
        icon: Zap,
        label: t('menu_automation') || 'Automation',
        children: [
          { id: 'macros', label: t('section_macros') || 'Macros' },
          { id: 'scripts', label: t('section_scripts') || 'Scripts' }
        ]
      },
      {
        id: 'settings',
        icon: Settings,
        label: t('menu_settings') || 'Settings',
        children: [
          { id: 'presets', label: t('section_presets') || 'Presets' },
          { id: 'diagnostics', label: t('section_diagnostics') || 'Diagnostics' },
          { id: 'config', label: t('section_config') || 'Config' }
        ]
      }
    ],
    [t]
  );

  // На рельсе (свёрнутое состояние) раскрывать список некуда, поэтому клик по
  // группе ведёт на её первый раздел — иначе пункт выглядел бы нерабочим.
  const primaryTarget = (item: MenuItem) => item.target ?? item.children?.[0]?.id ?? item.id;

  const isItemActive = (item: MenuItem) =>
    item.children
      ? item.children.some((child) => child.id === activeSection)
      : activeSection === (item.target ?? item.id);

  return (
    <>
      {sidebarMobileOpen && (
        <div className="m3-scrim z-[90] md:hidden" onClick={onCloseMobile} />
      )}

      {/* Кнопка вызова меню — только на узком экране. */}
      <button
        className="m3-icon-btn m3-state m3-icon-btn--outlined fixed left-4 top-4 z-[95] shadow-e2 md:hidden"
        onClick={onOpenMobile}
        aria-label={t('menu_open') || 'Open menu'}
        type="button"
      >
        <Menu />
      </button>

      <aside
        className={cn(
          'm3-nav fixed inset-y-0 left-0 z-[100]',
          // Ширину анимируем, а не layout вокруг: панель — отдельный слой,
          // и её изменение не заставляет пересчитывать содержимое страницы.
          'transition-[width,transform] duration-medium ease-emphasized',
          sidebarOpen ? 'w-[var(--app-drawer-width)]' : 'w-[var(--app-rail-width)]',
          'max-md:w-[288px]',
          sidebarMobileOpen ? 'max-md:translate-x-0' : 'max-md:-translate-x-full'
        )}
      >
        {/* ── Шапка ───────────────────────────────────────────────────── */}
        <div
          className={cn(
            'flex h-[var(--app-topbar-height)] shrink-0 items-center gap-3 px-4',
            !sidebarOpen && 'md:justify-center md:px-0'
          )}
        >
          {(sidebarOpen || sidebarMobileOpen) && (
            <>
              <span className="grid h-10 w-10 shrink-0 place-items-center rounded-m3md bg-primary-container text-on-primary-container">
                <Smartphone className="h-5 w-5" />
              </span>
              <span className="m3-title-medium min-w-0 flex-1 truncate">
                MK DroidScreenCast
              </span>
            </>
          )}

          <button
            className="m3-icon-btn m3-state hidden md:inline-flex"
            onClick={onToggleSidebar}
            aria-label={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
            type="button"
          >
            {sidebarOpen ? <PanelLeftClose /> : <PanelLeftOpen />}
          </button>

          <button
            className="m3-icon-btn m3-state md:hidden"
            onClick={onCloseMobile}
            aria-label="Close menu"
            type="button"
          >
            <X />
          </button>
        </div>

        {/* ── Навигация ───────────────────────────────────────────────── */}
        <nav className="min-h-0 flex-1 overflow-y-auto px-3 py-2">
          <ul className="flex flex-col gap-1">
            {menuItems.map((item) => {
              const Icon = item.icon;
              const active = isItemActive(item);
              const expanded = Boolean(expandedMenuGroups[item.id]);
              const showRail = !sidebarOpen && !sidebarMobileOpen;

              if (showRail) {
                return (
                  <li key={item.id} className="md:block hidden">
                    <button
                      className={cn(
                        'm3-rail-item m3-state',
                        active && 'm3-rail-item--active'
                      )}
                      onClick={() => onNavigate(primaryTarget(item))}
                      type="button"
                      aria-current={active ? 'page' : undefined}
                    >
                      <span className="m3-rail-item__indicator">
                        <Icon />
                      </span>
                      <span className="w-full truncate px-1 text-center" title={item.label}>
                        {item.label}
                      </span>
                    </button>
                  </li>
                );
              }

              return (
                <li key={item.id}>
                  <button
                    className={cn(
                      'm3-nav-item m3-state',
                      active && 'm3-nav-item--active',
                      !active && item.children && 'm3-nav-item--branch'
                    )}
                    onClick={() =>
                      item.children ? onToggleMenuGroup(item.id) : onNavigate(primaryTarget(item))
                    }
                    aria-expanded={item.children ? expanded : undefined}
                    type="button"
                  >
                    <Icon />
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    {item.children && (
                      <ChevronRight
                        className={cn(
                          'h-4 w-4 shrink-0 transition-transform duration-medium ease-spring-fast',
                          expanded && 'rotate-90'
                        )}
                      />
                    )}
                  </button>

                  {item.children && expanded && (
                    <ul className="m3-collapse-in mt-1 flex flex-col gap-0.5 pl-4">
                      {item.children.map((child) => {
                        const childActive = activeSection === child.id;
                        return (
                          <li key={child.id}>
                            <button
                              className={cn(
                                'm3-nav-sub m3-state',
                                childActive && 'm3-nav-sub--active'
                              )}
                              onClick={() => onNavigate(child.id)}
                              type="button"
                              aria-current={childActive ? 'page' : undefined}
                            >
                              <span
                                className={cn(
                                  'h-1.5 w-1.5 shrink-0 rounded-full bg-current transition-opacity',
                                  childActive ? 'opacity-100' : 'opacity-45'
                                )}
                              />
                              <span className="truncate">{child.label}</span>
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </li>
              );
            })}
          </ul>
        </nav>

        {/* ── Подвал: связь и тема ────────────────────────────────────── */}
        <div className="shrink-0 border-t border-outline-variant p-3">
          {sidebarOpen || sidebarMobileOpen ? (
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2.5 rounded-m3md bg-surface-container px-3 py-2.5">
                <span className={cn('m3-dot', !wsConnected && 'm3-dot--offline')} />
                <span className="m3-label-medium m3-on-variant truncate">
                  {wsConnected ? t('status_online') : t('status_offline')}
                </span>
              </div>

              {/* Сегментированный переключатель вместо <select>: три состояния
                  видно сразу, а выбор — одно нажатие вместо двух. */}
              <div
                className="m3-btn-group w-full"
                role="group"
                aria-label={t('theme_label') || 'Theme'}
              >
                {THEME_OPTIONS.map((option) => {
                  const OptionIcon = option.icon;
                  const selected = themePreference === option.value;
                  return (
                    <button
                      key={option.value}
                      className={cn(
                        'm3-btn m3-state m3-btn--xs flex-1 px-0',
                        selected ? 'm3-btn--tonal-primary' : 'm3-btn--outlined'
                      )}
                      onClick={() => onThemePreferenceChange(option.value)}
                      aria-pressed={selected}
                      title={t(option.labelKey) || option.fallback}
                      type="button"
                    >
                      <OptionIcon />
                    </button>
                  );
                })}
              </div>
            </div>
          ) : (
            <div className="flex flex-col items-center gap-3">
              <span
                className={cn('m3-dot', !wsConnected && 'm3-dot--offline')}
                title={wsConnected ? t('status_online') : t('status_offline')}
              />
              <button
                className="m3-icon-btn m3-state m3-icon-btn--outlined"
                onClick={onToggleTheme}
                title={themeLabel}
                aria-label={themeLabel}
                type="button"
              >
                {theme === 'dark' ? <Moon /> : <Sun />}
              </button>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}

/**
 * Меню перерисовывается только когда меняются его собственные пропсы.
 *
 * Родитель обновляется часто — тикающий таймер записи, опрос устройств,
 * прогресс загрузки, — и без memo вся эта навигация пересобиралась бы
 * несколько раз в секунду впустую.
 */
export const Sidebar = memo(SidebarView);
