import { expect as baseExpect, test as base } from '@playwright/test';

import { makeDeterministicName } from './helpers/names';
import { getSeed, getReposDir } from './helpers/seed';

type E2EFixtures = {
  seed: number;
  reposDir: string;
  makeName: (prefix: string) => string;
};

export const test = base.extend<E2EFixtures>({
  seed: [
    async ({}, use) => {
      await use(getSeed());
    },
    { scope: 'worker' },
  ],
  reposDir: [
    async ({}, use) => {
      await use(getReposDir());
    },
    { scope: 'worker' },
  ],
  makeName: async ({ seed }, use, testInfo) => {
    const scope = testInfo.titlePath.join(' > ');
    await use((prefix: string) => makeDeterministicName({ seed, scope, prefix }));
  },
});

export const expect = baseExpect;
