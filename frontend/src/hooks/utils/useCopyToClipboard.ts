import { useCallback } from 'react';

import { toast } from '@/components/ui/toast';

export function useCopyToClipboard() {
  return useCallback((label: string, value: string) => {
    if (!navigator.clipboard?.writeText) {
      toast({
        variant: 'destructive',
        title: 'Copy failed',
        description: `Clipboard API is not available.`,
      });
      return;
    }

    void navigator.clipboard
      .writeText(value)
      .then(() => {
        toast({
          title: 'Copied',
          description: `${label} copied to clipboard.`,
        });
      })
      .catch((err) => {
        console.error('Failed to copy to clipboard:', err);
        toast({
          variant: 'destructive',
          title: 'Copy failed',
          description: `Could not copy ${label}.`,
        });
      });
  }, []);
}
