module.exports = {
  plugins: {
    // Раскрывает @import до Tailwind. Без него Vite поднимает все @import в
    // начало файла, и наш базовый слой оказался бы раньше preflight — то есть
    // был бы им затёрт.
    'postcss-import': {},
    tailwindcss: {},
    autoprefixer: {}
  }
};
