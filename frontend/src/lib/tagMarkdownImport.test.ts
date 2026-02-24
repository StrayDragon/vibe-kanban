import { describe, expect, it } from 'vitest';
import { parseTagMarkdown } from './tagMarkdownImport';

describe('parseTagMarkdown', () => {
  it('parses headings and captures content between headings', () => {
    const input = [
      '# @first',
      'First line',
      'Second line',
      '## @second',
      'Second content',
      '',
      'Trailing line',
    ].join('\n');

    expect(parseTagMarkdown(input)).toEqual([
      {
        tagName: 'first',
        content: ['First line', 'Second line'].join('\n'),
      },
      {
        tagName: 'second',
        content: ['Second content', '', 'Trailing line'].join('\n'),
      },
    ]);
  });

  it('ignores headings without @tag syntax', () => {
    const input = ['# Heading', 'Body', '## @tag', 'Content'].join('\n');

    expect(parseTagMarkdown(input)).toEqual([
      {
        tagName: 'tag',
        content: 'Content',
      },
    ]);
  });

  it('strips markdown code fence markers from content', () => {
    const input = [
      '# @snippet',
      '```md',
      'Keep this line',
      '```',
      'After fence',
      'Inline ``` fence',
    ].join('\n');

    expect(parseTagMarkdown(input)).toEqual([
      {
        tagName: 'snippet',
        content: ['Keep this line', 'After fence', 'Inline  fence'].join('\n'),
      },
    ]);
  });

  it('dedupes by keeping the last occurrence and ordering by last appearance', () => {
    const input = [
      '# @first',
      'Old content',
      '## @second',
      'Second content',
      '### @first',
      'New content',
    ].join('\n');

    expect(parseTagMarkdown(input)).toEqual([
      {
        tagName: 'second',
        content: 'Second content',
      },
      {
        tagName: 'first',
        content: 'New content',
      },
    ]);
  });
});
