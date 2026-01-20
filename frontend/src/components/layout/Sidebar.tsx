import { useMemo } from 'react';
import type { ComponentType } from 'react';
import {
  ChevronRight,
  Folder,
  Home,
  Menu,
  Moon,
  PanelLeft,
  PanelLeftClose,
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

type MenuItem = {
  id: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
  children?: { id: string; label: string }[];
};

export function Sidebar({
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
  const sidebarMenuItems: MenuItem[] = useMemo(
    () => [
      { id: 'home', icon: Home, label: t('menu_home') || 'Home' },
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

  return (
    <>
      {sidebarMobileOpen && (
        <div
          className="sidebar-overlay fixed inset-0 z-[90] bg-black/50 md:hidden"
          onClick={onCloseMobile}
        />
      )}

      <button
        className={cn(
          'fixed left-4 top-4 z-[95] md:hidden',
          'inline-flex h-10 w-10 items-center justify-center rounded-full',
          'border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)]',
          'shadow-[var(--shadow-1)] transition hover:shadow-[var(--shadow-2)]'
        )}
        onClick={onOpenMobile}
        aria-label="Open menu"
      >
        <Menu className="h-5 w-5" />
      </button>

      <aside
        className={cn(
          'sidebar fixed left-0 top-0 bottom-0 z-[100] flex flex-col',
          'border-r border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)]',
          'transition-all duration-300 ease-out',
          sidebarOpen ? 'w-[260px]' : 'w-[60px]',
          'max-md:w-[280px]',
          sidebarMobileOpen ? 'max-md:translate-x-0' : 'max-md:-translate-x-full'
        )}
      >
        <div className="flex items-center justify-between gap-2 border-b border-[var(--md-sys-color-outline-variant)] px-4 py-4">
          {sidebarOpen && (
            <div className="flex items-center gap-3 overflow-hidden">
              <div className="grid h-9 w-9 shrink-0 place-items-center rounded-[12px] bg-[var(--md-sys-color-primary-container)] text-[var(--md-sys-color-on-primary-container)]">
                <Smartphone className="h-5 w-5" />
              </div>
              <span className="truncate font-display text-sm font-semibold">MK DroidScreenCast</span>
            </div>
          )}

          <button
            className={cn(
              'hidden md:flex h-8 w-8 items-center justify-center rounded-full',
              'border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)]',
              'text-[var(--md-sys-color-on-surface-variant)] transition hover:bg-[var(--md-sys-color-surface-container-high)]',
              !sidebarOpen && 'mx-auto'
            )}
            onClick={onToggleSidebar}
            aria-label={sidebarOpen ? 'Collapse sidebar' : 'Expand sidebar'}
          >
            {sidebarOpen ? <PanelLeftClose className="h-4 w-4" /> : <PanelLeft className="h-4 w-4" />}
          </button>

          <button
            className={cn(
              'md:hidden h-8 w-8 items-center justify-center rounded-full',
              'border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)]',
              'text-[var(--md-sys-color-on-surface-variant)]'
            )}
            onClick={onCloseMobile}
            aria-label="Close menu"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <nav className="flex-1 overflow-y-auto px-3 py-4">
          <ul className="flex flex-col gap-1">
            {sidebarMenuItems.map((item) => {
              const IconComponent = item.icon;
              const hasChildren = item.children && item.children.length > 0;
              const isExpanded = expandedMenuGroups[item.id];
              const isActive = !hasChildren && activeSection === item.id;
              const hasActiveChild = hasChildren && item.children?.some((c) => activeSection === c.id);

              return (
                <li key={item.id}>
                  <button
                    className={cn(
                      'flex w-full items-center gap-3 rounded-[var(--radius-sm)] px-3 py-2.5 text-sm font-medium',
                      'transition-colors duration-150',
                      isActive || hasActiveChild
                        ? 'bg-[var(--md-sys-color-primary-container)] text-[var(--md-sys-color-on-primary-container)]'
                        : 'text-[var(--md-sys-color-on-surface-variant)] hover:bg-[var(--md-sys-color-surface-container-high)]',
                      !sidebarOpen && 'justify-center px-2'
                    )}
                    onClick={() => {
                      if (hasChildren) {
                        onToggleMenuGroup(item.id);
                      } else {
                        onNavigate(item.id === 'home' ? 'faq' : item.id);
                      }
                    }}
                    title={!sidebarOpen ? item.label : undefined}
                  >
                    <IconComponent className="h-5 w-5 shrink-0" />
                    {sidebarOpen && (
                      <>
                        <span className="flex-1 truncate text-left">{item.label}</span>
                        {hasChildren && (
                          <ChevronRight
                            className={cn(
                              'h-4 w-4 shrink-0 transition-transform duration-200',
                              isExpanded && 'rotate-90'
                            )}
                          />
                        )}
                      </>
                    )}
                  </button>

                  {hasChildren && sidebarOpen && isExpanded && (
                    <ul className="mt-1 flex flex-col gap-0.5 pl-6">
                      {item.children!.map((child) => {
                        const isChildActive = activeSection === child.id;
                        return (
                          <li key={child.id}>
                            <button
                              className={cn(
                                'flex w-full items-center gap-2 rounded-[var(--radius-sm)] px-3 py-2 text-xs font-medium',
                                'transition-colors duration-150',
                                isChildActive
                                  ? 'bg-[var(--md-sys-color-secondary-container)] text-[var(--md-sys-color-on-secondary-container)]'
                                  : 'text-[var(--md-sys-color-on-surface-variant)] hover:bg-[var(--md-sys-color-surface-container)]'
                              )}
                              onClick={() => onNavigate(child.id)}
                            >
                              <span className="h-1.5 w-1.5 rounded-full bg-current opacity-50" />
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

        <div className="border-t border-[var(--md-sys-color-outline-variant)] px-3 py-3">
          {sidebarOpen ? (
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-2 rounded-[var(--radius-sm)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-xs">
                <span className={cn('status-dot h-2 w-2', !wsConnected && 'offline')} />
                <span className="text-[var(--md-sys-color-on-surface-variant)]">
                  {wsConnected ? t('status_online') : t('status_offline')}
                </span>
              </div>

              <select
                className="w-full rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-xs text-[var(--md-sys-color-on-surface)] focus:outline-none"
                value={themePreference}
                onChange={(e) => onThemePreferenceChange(e.target.value as 'auto' | 'light' | 'dark')}
              >
                <option value="auto">{t('theme_system') || 'System'}</option>
                <option value="light">{t('theme_light') || 'Light'}</option>
                <option value="dark">{t('theme_dark') || 'Dark'}</option>
              </select>
            </div>
          ) : (
            <button
              className="flex h-9 w-9 items-center justify-center rounded-full border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] mx-auto"
              onClick={onToggleTheme}
              title={themeLabel}
            >
              {theme === 'dark' ? <Moon className="h-4 w-4" /> : <Sun className="h-4 w-4" />}
            </button>
          )}
        </div>
      </aside>
    </>
  );
}
