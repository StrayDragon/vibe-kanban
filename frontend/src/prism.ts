import prism from 'prismjs';

type GlobalWithPrism = typeof globalThis & { Prism: typeof prism };

(globalThis as GlobalWithPrism).Prism = prism;
