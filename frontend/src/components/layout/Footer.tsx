import { memo } from 'react';
import { Github, Send } from 'lucide-react';

type FooterProps = {
  t: (key: string) => string;
  version?: string;
};

function FooterView({ t, version }: FooterProps) {
  return (
    <footer className="flex flex-col items-center gap-3 pb-2 pt-2 text-center">
      <hr className="m3-divider w-full" />
      <p className="m3-body-small m3-on-variant">{t('footer_notice')}</p>
      <div className="flex flex-wrap items-center justify-center gap-2">
        <a
          className="m3-btn m3-state m3-btn--text m3-btn--sm"
          href="https://github.com/MKultra6969"
          target="_blank"
          rel="noreferrer"
        >
          <Github />
          GitHub
        </a>
        <a
          className="m3-btn m3-state m3-btn--text m3-btn--sm"
          href="https://t.me/MKplusULTRA"
          target="_blank"
          rel="noreferrer"
        >
          <Send />
          Telegram
        </a>
        {version && <span className="m3-badge">v{version}</span>}
      </div>
    </footer>
  );
}

export const Footer = memo(FooterView);
