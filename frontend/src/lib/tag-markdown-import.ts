export type ParsedTagEntry = {
  tagName: string;
  content: string;
};

const headingRegex = /^#{1,6}\s+@([^\s#]+)\s*(?:$|.*)$/;

const stripCodeFenceLines = (lines: string[]) =>
  lines.filter((line) => !line.trim().startsWith('```'));

const stripFenceMarkers = (content: string) => content.replace(/```+/g, '');

export function parseTagMarkdown(input: string): ParsedTagEntry[] {
  const lines = input.split(/\r?\n/);
  const entries: ParsedTagEntry[] = [];

  let current: { tagName: string; contentLines: string[] } | null = null;

  const flush = () => {
    if (!current) return;
    const cleanedLines = stripCodeFenceLines(current.contentLines);
    entries.push({
      tagName: current.tagName,
      content: stripFenceMarkers(cleanedLines.join('\n')).trim(),
    });
  };

  for (const line of lines) {
    const match = line.match(headingRegex);
    if (match) {
      flush();
      current = {
        tagName: match[1],
        contentLines: [],
      };
      continue;
    }

    if (current) {
      current.contentLines.push(line);
    }
  }

  flush();

  const deduped = new Map<string, ParsedTagEntry>();
  for (const entry of entries) {
    if (deduped.has(entry.tagName)) {
      deduped.delete(entry.tagName);
    }
    deduped.set(entry.tagName, entry);
  }

  return Array.from(deduped.values());
}
