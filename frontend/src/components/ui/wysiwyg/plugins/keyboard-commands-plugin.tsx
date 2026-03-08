import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { $convertToMarkdownString, type Transformer } from '@lexical/markdown';
import {
  KEY_MODIFIER_COMMAND,
  KEY_ENTER_COMMAND,
  COMMAND_PRIORITY_NORMAL,
  COMMAND_PRIORITY_HIGH,
} from 'lexical';

type Props = {
  transformers: Transformer[];
  onCmdEnter?: (markdown: string) => void;
  onShiftCmdEnter?: (markdown: string) => void;
};

export function KeyboardCommandsPlugin({
  transformers,
  onCmdEnter,
  onShiftCmdEnter,
}: Props) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    if (!onCmdEnter && !onShiftCmdEnter) return;

    // Handle the modifier command to trigger the callbacks
    const unregisterModifier = editor.registerCommand(
      KEY_MODIFIER_COMMAND,
      (event: KeyboardEvent) => {
        if (!(event.metaKey || event.ctrlKey) || event.key !== 'Enter') {
          return false;
        }

        event.preventDefault();
        event.stopPropagation();

        const markdown = editor.getEditorState().read(() =>
          $convertToMarkdownString(transformers)
        );

        if (event.shiftKey && onShiftCmdEnter) {
          onShiftCmdEnter(markdown);
          return true;
        }

        if (!event.shiftKey && onCmdEnter) {
          onCmdEnter(markdown);
          return true;
        }

        return false;
      },
      COMMAND_PRIORITY_NORMAL
    );

    // Block KEY_ENTER_COMMAND when CMD/Ctrl is pressed to prevent
    // RichTextPlugin from inserting a new line
    const unregisterEnter = editor.registerCommand(
      KEY_ENTER_COMMAND,
      (event: KeyboardEvent | null) => {
        if (event && (event.metaKey || event.ctrlKey)) {
          return true; // Mark as handled, preventing line break insertion
        }
        return false;
      },
      COMMAND_PRIORITY_HIGH
    );

    return () => {
      unregisterModifier();
      unregisterEnter();
    };
  }, [editor, onCmdEnter, onShiftCmdEnter, transformers]);

  return null;
}
