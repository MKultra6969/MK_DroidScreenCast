import { memo } from 'react';
import { Languages, Smartphone } from 'lucide-react';
import { delayStyle } from '../../lib/style';

type HeaderProps = {
  t: (key: string) => string;
  lang: string;
  languages: string[];
  onLanguageChange: (value: string) => void;
};

function HeaderView({ t, lang, languages, onLanguageChange }: HeaderProps) {
  return (
    <header
      className="m3-enter relative overflow-hidden rounded-m3xxl bg-surface-low p-6 shadow-e1 sm:p-8"
      style={delayStyle(0)}
    >
      {/*
        Акцентная подсветка — один статичный градиент. Раньше здесь было три
        слоя с рамками и бесконечной анимацией; выглядело так же, а стоило
        перерисовки шапки на каждом кадре. Радиальные пятна вместо кругов с
        `filter: blur()`: мягкий край получается из самого градиента, растровый
        проход не нужен.
      */}
      <div aria-hidden className="app-hero-glow" />

      <div className="relative flex flex-wrap items-center justify-between gap-6">
        <div className="flex min-w-0 items-center gap-4">
          <span className="grid h-14 w-14 shrink-0 place-items-center rounded-m3lg bg-primary text-on-primary shadow-e2">
            <Smartphone className="h-7 w-7" />
          </span>
          <div className="min-w-0">
            <h1 className="m3-headline-medium truncate">{t('title')}</h1>
            <p className="m3-body-medium m3-on-variant mt-1">{t('subtitle')}</p>
          </div>
        </div>

        <label className="flex items-center gap-2.5">
          <span className="m3-field-label">
            <Languages />
            <span className="sr-only sm:not-sr-only">{t('language_label')}</span>
          </span>
          <select
            className="m3-field m3-select m3-field--sm w-[92px]"
            value={lang}
            onChange={(event) => onLanguageChange(event.target.value)}
            aria-label={t('language_label')}
          >
            {languages.map((item) => (
              <option key={item} value={item}>
                {item.toUpperCase()}
              </option>
            ))}
          </select>
        </label>
      </div>
    </header>
  );
}

export const Header = memo(HeaderView);
