import type { CSSProperties } from 'react';

export const delayStyle = (delay: number) =>
  ({
    '--delay': `${delay}ms`
  }) as CSSProperties;
