import { Github, Send } from 'lucide-react';

type FooterProps = {
  t: (key: string) => string;
  version?: string;
};

export function Footer({ t, version }: FooterProps) {
  return (
    <footer className="flex flex-col items-center gap-3 border-t border-[var(--md-sys-color-outline-variant)] pt-4 text-center text-sm text-[var(--md-sys-color-on-surface-variant)]">
      <p>{t('footer_notice')}</p>
      <div className="flex flex-wrap justify-center gap-6">
        <a
          href="https://github.com/MKultra6969"
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-2 font-semibold text-[var(--md-sys-color-primary)] hover:underline"
        >
          <Github className="h-4 w-4" />
          GitHub
        </a>
        <a
          href="https://t.me/MKplusULTRA"
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-2 font-semibold text-[var(--md-sys-color-primary)] hover:underline"
        >
          <Send className="h-4 w-4" />
          Telegram
        </a>
      </div>
      {version ? (
        <div className="text-xs font-semibold text-[var(--md-sys-color-on-surface-variant)]">
          v{version}
        </div>
      ) : null}
    </footer>
  );
}
