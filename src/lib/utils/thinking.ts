export function summarizeThinking(content: string): string {
  const trimmed = content.trim();
  if (!trimmed) return '';

  const boldPrefixMatch = trimmed.match(/^\*\*([^*]+)\*\*/);
  if (!boldPrefixMatch) {
    return trimmed;
  }

  const title = boldPrefixMatch[1]?.trim();
  return title || trimmed;
}

