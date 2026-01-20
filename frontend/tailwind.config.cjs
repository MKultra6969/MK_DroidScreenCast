/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        ui: ['var(--font-ui)', 'sans-serif'],
        display: ['var(--font-display)', 'sans-serif']
      },
      keyframes: {
        fadeUp: {
          to: { opacity: '1', transform: 'translateY(0) scale(1)', filter: 'blur(0)' }
        },
        cardIn: {
          from: { opacity: '0', transform: 'translateY(12px) scale(0.98)' },
          to: { opacity: '1', transform: 'translateY(0) scale(1)' }
        },
        listIn: {
          from: { opacity: '0', transform: 'translateY(8px)' },
          to: { opacity: '1', transform: 'translateY(0)' }
        },
        toastIn: {
          from: { opacity: '0', transform: 'translateY(-12px)' },
          to: { opacity: '1', transform: 'translateY(0)' }
        },
        gradientShift: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(12px)' }
        },
        floatSlow: {
          '0%, 100%': { transform: 'translateY(0) translateX(0)' },
          '50%': { transform: 'translateY(24px) translateX(12px)' }
        },
        pulseSoft: {
          '0%, 100%': { transform: 'scale(1)', opacity: '0.9' },
          '50%': { transform: 'scale(1.3)', opacity: '0.6' }
        }
      },
      animation: {
        'fade-up': 'fadeUp 0.7s ease forwards',
        'card-in': 'cardIn 0.5s ease',
        'list-in': 'listIn 0.4s ease',
        'toast-in': 'toastIn 0.35s ease',
        'gradient-shift': 'gradientShift 24s ease-in-out infinite',
        'float-slow': 'floatSlow 18s ease-in-out infinite',
        'pulse-soft': 'pulseSoft 2.4s ease-in-out infinite'
      }
    }
  },
  plugins: []
};