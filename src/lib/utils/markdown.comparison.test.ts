import { describe, it, expect } from 'vitest';
import { marked } from 'marked';
import DOMPurify from 'dompurify';
import { renderMarkdown } from './markdown';

// Simulate the OLD ChatContainer streaming path: raw marked.parse + DOMPurify, no hljs
function oldStreamingRender(content: string): string {
  return DOMPurify.sanitize(marked.parse(content) as string);
}

describe('markdown rendering: old streaming vs new shared utility', () => {
  const testCases = [
    { name: 'plain text', input: 'Hello **world**' },
    { name: 'inline code', input: 'Use `console.log()` here' },
    { name: 'js code block', input: '```js\nconst x = 1;\nconsole.log(x);\n```' },
    { name: 'python code block', input: '```python\ndef hello():\n    print("hi")\n```' },
    { name: 'unfenced code block', input: '```\nplain code block\n```' },
    { name: 'heading + link', input: '# Heading\n\nSome text with [a link](https://example.com)' },
    { name: 'nested markdown', input: '- item 1\n- item 2\n  - nested\n- item 3' },
    { name: 'blockquote', input: '> This is a quote\n> with multiple lines' },
    { name: 'table', input: '| a | b |\n|---|---|\n| 1 | 2 |' },
  ];

  // Note: after our change, both paths use the same renderMarkdown().
  // But marked is a global singleton - the old path would have called marked.parse()
  // WITHOUT the custom renderer. Since our markdown.ts imports configure marked globally
  // with the hljs renderer, calling marked.parse() anywhere now uses hljs.
  // This test verifies that the import side-effect works correctly.

  it('renderMarkdown produces valid HTML for all test cases', () => {
    for (const tc of testCases) {
      const result = renderMarkdown(tc.input);
      expect(result, `Failed for: ${tc.name}`).toBeTruthy();
      expect(result, `Empty output for: ${tc.name}`).not.toBe('');
    }
  });

  it('code blocks get hljs classes (the key fix)', () => {
    const result = renderMarkdown('```js\nconst x = 1;\n```');
    expect(result).toContain('class="hljs language-js"');
    // This is exactly what the old streaming path was MISSING
  });

  it('old streaming path now also gets hljs (because marked is global)', () => {
    // After importing markdown.ts, marked is globally configured with hljs renderer
    // So even calling marked.parse() directly would now use hljs
    const result = oldStreamingRender('```js\nconst x = 1;\n```');
    expect(result).toContain('class="hljs language-js"');
  });

  it('non-code content is identical between old and new paths', () => {
    const plainCases = [
      'Hello **world**',
      'Use `console.log()` here',
      '# Heading\n\nParagraph',
      '- item 1\n- item 2',
      '> blockquote',
    ];
    for (const input of plainCases) {
      const newResult = renderMarkdown(input);
      const oldResult = oldStreamingRender(input);
      expect(newResult, `Mismatch for: ${input}`).toBe(oldResult);
    }
  });

  it('code block output is identical between direct marked.parse and renderMarkdown', () => {
    // Since marked is globally configured, both should be identical
    const input = '```python\ndef hello():\n    print("hi")\n```';
    const newResult = renderMarkdown(input);
    const oldResult = oldStreamingRender(input);
    expect(newResult).toBe(oldResult);
  });

  it('XSS is sanitized in code blocks', () => {
    const result = renderMarkdown('```html\n<script>alert("xss")</script>\n```');
    expect(result).not.toContain('<script>');
  });

  it('handles empty and edge cases gracefully', () => {
    expect(renderMarkdown('')).toBe('');
    expect(renderMarkdown('   ')).toBeDefined();
    expect(renderMarkdown('\n\n')).toBeDefined();
  });
});
