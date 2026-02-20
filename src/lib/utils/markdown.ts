import { marked } from 'marked';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js';

const renderer = new marked.Renderer();
renderer.code = ({ text, lang }: { text: string; lang?: string }) => {
  const language = lang && hljs.getLanguage(lang) ? lang : 'plaintext';
  const highlighted = hljs.highlight(text, { language }).value;
  return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
};

marked.use({ renderer, async: false });

export function renderMarkdown(content: string): string {
  return DOMPurify.sanitize(marked.parse(content) as string);
}
