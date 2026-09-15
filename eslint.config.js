// Flat config, shared by every workspace package.
import eslint from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    rules: {
      // Agents move real money on a testnet. An ignored promise is a dropped payment.
      '@typescript-eslint/no-floating-promises': 'error',
      '@typescript-eslint/no-misused-promises': 'error',
      // Amounts are bigint base units, never number. This catches the accidental Number().
      '@typescript-eslint/no-unsafe-assignment': 'error',
      'no-restricted-globals': [
        'error',
        { name: 'Math', message: 'No float math on amounts. Use bigint base units.' },
      ],
    },
  },
  { ignores: ['**/dist/**', '**/node_modules/**', 'contracts/**', 'engine/**'] },
);
