import path from 'node:path';
import { fileURLToPath } from 'node:url';

import tseslintPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import checkFile from 'eslint-plugin-check-file';
import eslintComments from 'eslint-plugin-eslint-comments';
import i18next from 'eslint-plugin-i18next';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import unusedImports from 'eslint-plugin-unused-imports';
import prettier from 'eslint-config-prettier';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const i18nCheck = process.env.LINT_I18N === 'true';

const restrictedSyntaxModal = [
  {
    selector:
      'CallExpression[callee.object.name="NiceModal"][callee.property.name="show"]',
    message:
      'Do not use NiceModal.show() directly. Use DialogName.show(props) instead.',
  },
  {
    selector:
      'CallExpression[callee.object.name="NiceModal"][callee.property.name="register"]',
    message:
      'Do not use NiceModal.register(). Dialogs are registered automatically.',
  },
  {
    selector: 'CallExpression[callee.name="showModal"]',
    message: 'Do not use showModal(). Use DialogName.show(props) instead.',
  },
  {
    selector: 'CallExpression[callee.name="hideModal"]',
    message: 'Do not use hideModal(). Use DialogName.hide() instead.',
  },
  {
    selector: 'CallExpression[callee.name="removeModal"]',
    message: 'Do not use removeModal(). Use DialogName.remove() instead.',
  },
];

const restrictedSyntaxNetwork = [
  {
    selector: 'CallExpression[callee.name="fetch"]',
    message:
      'Do not call fetch() outside frontend/src/api/**. Use the API layer (e.g. makeRequest/handleApiResponse) instead.',
  },
  {
    selector: 'NewExpression[callee.name="WebSocket"]',
    message:
      'Do not construct WebSocket outside frontend/src/api/**. Use createWebSocket() from the API layer instead.',
  },
  {
    selector: 'NewExpression[callee.name="EventSource"]',
    message:
      'Do not construct EventSource outside frontend/src/api/**. Use createEventSource() from the API layer instead.',
  },
];

const restrictedSyntaxQueryKeys = [
  {
    selector: 'Property[key.name="queryKey"] > ArrayExpression',
    message:
      'Do not inline React Query queryKey arrays in hooks. Define/import a domain key factory (e.g. *Keys) and use that instead.',
  },
];

const baseParserOptions = {
  ecmaVersion: 'latest',
  sourceType: 'module',
};

const typeAwareParserOptions = {
  ...baseParserOptions,
  project: ['./tsconfig.json'],
  tsconfigRootDir: __dirname,
};

const tsRecommendedRules = Object.assign(
  {},
  ...tseslintPlugin.configs['flat/recommended'].map((config) => config.rules ?? {})
);

const i18nextRecommendedRules = i18next.configs['flat/recommended'].rules;
const eslintCommentsRecommendedRules = eslintComments.configs.recommended.rules;

const plugins = {
  '@typescript-eslint': tseslintPlugin,
  'check-file': checkFile,
  'eslint-comments': eslintComments,
  i18next,
  'react-hooks': reactHooks,
  'react-refresh': reactRefresh,
  'unused-imports': unusedImports,
};

export default [
  {
    ignores: ['dist/**'],
  },
  {
    plugins,
  },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: typeAwareParserOptions,
    },
    rules: {
      ...tsRecommendedRules,
      ...i18nextRecommendedRules,
      ...eslintCommentsRecommendedRules,
      ...prettier.rules,
      'eslint-comments/no-use': ['error', { allow: [] }],
      'react-refresh/only-export-components': 'off',
      'unused-imports/no-unused-imports': 'error',
      'unused-imports/no-unused-vars': [
        'error',
        {
          vars: 'all',
          args: 'after-used',
          ignoreRestSiblings: false,
          caughtErrors: 'none',
        },
      ],
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-empty-object-type': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          args: 'after-used',
          caughtErrors: 'none',
        },
      ],
      '@typescript-eslint/switch-exhaustiveness-check': 'error',
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      // Enforce typesafe modal pattern
      'no-restricted-imports': [
        'error',
        {
          paths: [
            {
              name: '@ebay/nice-modal-react',
              importNames: ['default'],
              message:
                'Import NiceModal only in lib/modals.ts or dialog component files. Use DialogName.show(props) instead.',
            },
            {
              name: '@/lib/modals',
              importNames: ['showModal', 'hideModal', 'removeModal'],
              message:
                'Do not import showModal/hideModal/removeModal. Use DialogName.show(props) and DialogName.hide() instead.',
            },
          ],
        },
      ],
      'no-restricted-syntax': [
        'error',
        ...restrictedSyntaxModal,
        ...restrictedSyntaxNetwork,
        ...restrictedSyntaxQueryKeys,
      ],
      // i18n rule - only active when LINT_I18N=true
      'i18next/no-literal-string': i18nCheck
        ? [
            'warn',
            {
              markupOnly: true,
              ignoreAttribute: [
                'data-testid',
                'to',
                'href',
                'id',
                'key',
                'type',
                'role',
                'className',
                'style',
                'aria-describedby',
              ],
              'jsx-components': {
                exclude: ['code'],
              },
            },
          ]
        : 'off',
      // File naming conventions
      'check-file/filename-naming-convention': [
        'error',
        {
          // React components (tsx) should be PascalCase
          'src/**/*.tsx': 'PASCAL_CASE',
          // Hooks should be camelCase starting with 'use'
          'src/**/use*.ts': 'CAMEL_CASE',
          // Utils should be camelCase
          'src/utils/**/*.ts': 'CAMEL_CASE',
          // Lib/config/constants should be camelCase
          'src/lib/**/*.ts': 'CAMEL_CASE',
          'src/config/**/*.ts': 'CAMEL_CASE',
          'src/constants/**/*.ts': 'CAMEL_CASE',
        },
        {
          ignoreMiddleExtensions: true,
        },
      ],
    },
  },
  {
    // Entry point exception - main.tsx can stay lowercase
    files: ['src/main.tsx', 'src/vite-env.d.ts'],
    rules: {
      'check-file/filename-naming-convention': 'off',
    },
  },
  {
    // Shadcn UI components are an exception - keep kebab-case
    files: ['src/components/ui/**/*.{ts,tsx}'],
    rules: {
      'check-file/filename-naming-convention': [
        'error',
        {
          'src/components/ui/**/*.{ts,tsx}': 'KEBAB_CASE',
        },
        {
          ignoreMiddleExtensions: true,
        },
      ],
    },
  },
  {
    files: ['**/*.test.{ts,tsx}', '**/*.stories.{ts,tsx}'],
    rules: {
      'i18next/no-literal-string': 'off',
    },
  },
  {
    // Disable type-aware linting for config files
    files: ['*.config.{ts,js,cjs,mjs}', 'eslint.config.{js,mjs,cjs}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ...baseParserOptions,
        project: null,
      },
    },
    rules: {
      '@typescript-eslint/switch-exhaustiveness-check': 'off',
    },
  },
  {
    // Allow NiceModal usage in lib/modals.ts, App.tsx (for Provider), and dialog component files
    files: [
      'src/lib/modals.ts',
      'src/App.tsx',
      'src/components/dialogs/**/*.{ts,tsx}',
    ],
    rules: {
      'no-restricted-imports': 'off',
      'no-restricted-syntax': [
        'error',
        ...restrictedSyntaxNetwork,
        ...restrictedSyntaxQueryKeys,
      ],
    },
  },
  {
    // API boundary allowlist: only API modules may call fetch / construct WS/SSE
    files: ['src/api/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-syntax': [
        'error',
        ...restrictedSyntaxModal,
        ...restrictedSyntaxQueryKeys,
      ],
    },
  },
  {
    // Guardrail: hooks must not inline queryKey arrays
    files: ['src/hooks/**/*.{ts,tsx}'],
    ignores: ['src/hooks/**/*.test.{ts,tsx}'],
    rules: {
      'no-restricted-syntax': [
        'error',
        ...restrictedSyntaxModal,
        ...restrictedSyntaxNetwork,
        ...restrictedSyntaxQueryKeys,
      ],
    },
  },
];
