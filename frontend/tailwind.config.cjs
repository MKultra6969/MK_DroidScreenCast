/** @type {import('tailwindcss').Config} */

/*
 * Tailwind здесь отвечает за раскладку и мелкие правки, а не за внешний вид:
 * цвета, формы и тени приходят из системных токенов M3 (см. src/styles).
 *
 * Роли M3 подняты в theme.extend, чтобы в разметке было `bg-surface-container`
 * вместо `bg-[var(--md-sys-color-surface-container)]`. Это не только короче
 * читается — произвольные значения Tailwind генерирует по одному классу на
 * каждое написание, а именованные переиспользует.
 */

const role = (name) => `var(--md-sys-color-${name})`;

module.exports = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        primary: role('primary'),
        'on-primary': role('on-primary'),
        'primary-container': role('primary-container'),
        'on-primary-container': role('on-primary-container'),

        secondary: role('secondary'),
        'on-secondary': role('on-secondary'),
        'secondary-container': role('secondary-container'),
        'on-secondary-container': role('on-secondary-container'),

        tertiary: role('tertiary'),
        'on-tertiary': role('on-tertiary'),
        'tertiary-container': role('tertiary-container'),
        'on-tertiary-container': role('on-tertiary-container'),

        error: role('error'),
        'on-error': role('on-error'),
        'error-container': role('error-container'),
        'on-error-container': role('on-error-container'),

        success: role('success'),
        'on-success': role('on-success'),
        'success-container': role('success-container'),
        'on-success-container': role('on-success-container'),

        background: role('background'),
        surface: role('surface'),
        'surface-dim': role('surface-dim'),
        'surface-bright': role('surface-bright'),
        'surface-lowest': role('surface-container-lowest'),
        'surface-low': role('surface-container-low'),
        'surface-container': role('surface-container'),
        'surface-high': role('surface-container-high'),
        'surface-highest': role('surface-container-highest'),
        'on-surface': role('on-surface'),
        'on-surface-variant': role('on-surface-variant'),

        outline: role('outline'),
        'outline-variant': role('outline-variant'),
        'inverse-surface': role('inverse-surface'),
        'inverse-on-surface': role('inverse-on-surface')
      },
      borderRadius: {
        m3xs: 'var(--md-sys-shape-xs)',
        m3sm: 'var(--md-sys-shape-sm)',
        m3md: 'var(--md-sys-shape-md)',
        m3lg: 'var(--md-sys-shape-lg)',
        m3xl: 'var(--md-sys-shape-xl)',
        m3xxl: 'var(--md-sys-shape-xl-increased)'
      },
      boxShadow: {
        e1: 'var(--md-sys-elevation-1)',
        e2: 'var(--md-sys-elevation-2)',
        e3: 'var(--md-sys-elevation-3)',
        e4: 'var(--md-sys-elevation-4)',
        e5: 'var(--md-sys-elevation-5)'
      },
      fontFamily: {
        ui: ['var(--md-sys-typescale-plain)', 'sans-serif'],
        display: ['var(--md-sys-typescale-brand)', 'sans-serif'],
        mono: ['var(--md-sys-typescale-mono)', 'monospace']
      },
      transitionTimingFunction: {
        emphasized: 'var(--md-sys-motion-easing-emphasized)',
        'emphasized-in': 'var(--md-sys-motion-easing-emphasized-accelerate)',
        'emphasized-out': 'var(--md-sys-motion-easing-emphasized-decelerate)',
        spring: 'var(--md-sys-motion-spring-default)',
        'spring-fast': 'var(--md-sys-motion-spring-fast)'
      },
      transitionDuration: {
        short: '150ms',
        medium: '300ms',
        long: '450ms'
      },
      maxWidth: {
        content: 'var(--app-content-max)'
      }
    }
  },
  plugins: []
};
