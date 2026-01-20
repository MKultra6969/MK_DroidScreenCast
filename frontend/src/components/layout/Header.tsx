import { Smartphone } from 'lucide-react';
import { delayStyle } from '../../lib/style';

type HeaderProps = {
  t: (key: string) => string;
  lang: string;
  languages: string[];
  onLanguageChange: (value: string) => void;
};

export function Header({ t, lang, languages, onLanguageChange }: HeaderProps) {
  return (
    <header className="hero reveal" style={delayStyle(0)}>
      <div className="relative z-10 flex flex-wrap items-center justify-between gap-6">
        <div className="flex flex-wrap items-center gap-4">
          <div className="grid h-14 w-14 place-items-center rounded-[18px] bg-[var(--md-sys-color-primary-container)] text-[var(--md-sys-color-on-primary-container)] shadow-[inset_0_0_0_1px_rgba(5,60,54,0.1)]">
            <Smartphone className="h-9 w-9" />
          </div>
          <div>
            <h1 className="font-display text-2xl font-bold">{t('title')}</h1>
            <p className="mt-1 text-sm text-[var(--md-sys-color-on-surface-variant)]">{t('subtitle')}</p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-4 text-sm text-[var(--md-sys-color-on-surface-variant)]">
          <div className="flex flex-col gap-1">
            <label className="text-xs" htmlFor="langSelect">
              {t('language_label')}
            </label>
            <select
              id="langSelect"
              className="w-24 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-xs text-[var(--md-sys-color-on-surface)] focus:border-[var(--md-sys-color-primary)] focus:outline-none focus:ring-2 focus:ring-[var(--accent-ring)]"
              value={lang}
              onChange={(event) => onLanguageChange(event.target.value)}
            >
              {languages.map((item) => (
                <option key={item} value={item}>
                  {item.toUpperCase()}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>
    </header>
  );
}
