import { describe, it, expect } from 'vitest';
import { renderMarkdown } from './markdown';

describe('renderMarkdown', () => {
  it('renders plain text as paragraph', () => {
    const html = renderMarkdown('hello world');
    expect(html).toContain('<p>hello world</p>');
  });

  it('renders inline code', () => {
    const html = renderMarkdown('use `console.log()`');
    expect(html).toContain('<code>console.log()</code>');
  });

  it('renders fenced code blocks with syntax highlighting', () => {
    const html = renderMarkdown('```js\nconst x = 1;\n```');
    expect(html).toContain('class="hljs language-js"');
    expect(html).toContain('<pre><code');
  });

  it('falls back to plaintext for unknown languages', () => {
    const html = renderMarkdown('```unknownlang\nfoo\n```');
    expect(html).toContain('class="hljs language-plaintext"');
  });

  it('renders code blocks without language as plaintext', () => {
    const html = renderMarkdown('```\nfoo bar\n```');
    expect(html).toContain('class="hljs language-plaintext"');
  });

  it('sanitizes dangerous HTML', () => {
    const html = renderMarkdown('<script>alert("xss")</script>');
    expect(html).not.toContain('<script>');
  });

  it('renders markdown links', () => {
    const html = renderMarkdown('[click](https://example.com)');
    expect(html).toContain('<a href="https://example.com"');
  });
});
