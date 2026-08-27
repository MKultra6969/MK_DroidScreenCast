import { memo } from 'react';
import { Github, Send } from 'lucide-react';

import { openExternal } from '../../lib/open';

type FooterProps = {
  t: (key: string) => string;
  version?: string;
};

/** Ссылки наружу — кнопки, а не `<a>`.
 *
 * У `<a target="_blank">` в webview Tauri нет никакого эффекта: открывать новые
 * окна движку не разрешено, и нажатие на GitHub и Telegram просто ничего не
 * делало. Системный браузер умеет открывать только плагин opener. */
const LINKS = [
  { id: 'github', icon: Github, label: 'GitHub', url: 'https://github.com/MKultra6969' },
  { id: 'telegram', icon: Send, label: 'Telegram', url: 'https://t.me/MKplusULTRA' }
] as const;

function FooterView({ t, version }: FooterProps) {
  return (
    <footer className="flex flex-col items-center gap-3 pb-2 pt-2 text-center">
      <hr className="m3-divider w-full" />
      <p className="m3-body-small m3-on-variant">{t('footer_notice')}</p>
      <div className="flex flex-wrap items-center justify-center gap-2">
        {LINKS.map(({ id, icon: Icon, label, url }) => (
          <button
            key={id}
            type="button"
            className="m3-btn m3-state m3-btn--text m3-btn--sm"
            onClick={() => void openExternal(url)}
            title={url}
          >
            <Icon />
            {label}
          </button>
        ))}
        {version && <span className="m3-badge">v{version}</span>}
      </div>
    </footer>
  );
}

export const Footer = memo(FooterView);
