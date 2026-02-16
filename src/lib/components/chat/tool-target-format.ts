export function getCompactToolTarget(toolType: string, target: string): string {
  if (toolType !== 'Thinking') {
    return target;
  }

  const boldPrefixMatch = target.match(/^\*\*([^*]+)\*\*(?:\s|$)/);
  if (!boldPrefixMatch) {
    return target;
  }

  return boldPrefixMatch[1]?.trim() || target;
}
