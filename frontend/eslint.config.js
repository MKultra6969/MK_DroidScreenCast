import js from '@eslint/js';
import globals from 'globals';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist', 'dist-tauri', 'node_modules', '../static'] },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],

      // The effect-dependency rule is what hid the duplicate `adb shell ls`
      // on every folder change, so it must fail the build, not just warn.
      'react-hooks/exhaustive-deps': 'error',

      // Unused code is caught by tsc's noUnusedLocals for values; this covers
      // the type-only cases tsc skips, and allows the _prefix escape hatch.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' }
      ],

      // readJson<T> and the API response shapes are deliberately loose.
      '@typescript-eslint/no-explicit-any': 'off',

      eqeqeq: ['error', 'smart'],
      'no-console': ['warn', { allow: ['warn', 'error'] }]
    }
  }
);
