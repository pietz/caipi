import { describe, expect, it } from 'vitest';
import { getCompactToolTarget } from './tool-target-format';

describe('getCompactToolTarget', () => {
  it('returns non-thinking targets unchanged', () => {
    expect(getCompactToolTarget('Read', '/tmp/file.txt')).toBe('/tmp/file.txt');
  });

  it('returns bold title prefix for thinking targets', () => {
    expect(getCompactToolTarget('Thinking', '**Plan** Step 1: inspect')).toBe('Plan');
  });

  it('returns full thinking text when there is no bold prefix', () => {
    const summary = 'Inspecting project structure and dependencies';
    expect(getCompactToolTarget('Thinking', summary)).toBe(summary);
  });
});
