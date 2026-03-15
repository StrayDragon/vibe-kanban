import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

if (!HTMLElement.prototype.scrollIntoView) {
  HTMLElement.prototype.scrollIntoView = function scrollIntoView() {};
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
