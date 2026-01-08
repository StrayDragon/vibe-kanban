import type { LogStreamEvent, PatchType } from 'shared/types';

interface StreamOptions {
  onAppend?: (entryIndex: bigint, entry: PatchType) => void;
  onReplace?: (entryIndex: bigint, entry: PatchType) => void;
  onFinished?: () => void;
  onError?: (err: unknown) => void;
  onOpen?: () => void;
}

interface StreamController {
  close(): void;
  isConnected(): boolean;
}

export function streamLogEntries(
  url: string,
  opts: StreamOptions = {}
): StreamController {
  const wsUrl = url.replace(/^http/, 'ws');
  const ws = new WebSocket(wsUrl);
  let connected = false;
  let closedByClient = false;
  let finished = false;
  let sawError = false;

  ws.addEventListener('open', () => {
    connected = true;
    opts.onOpen?.();
  });

  ws.addEventListener('message', (event) => {
    try {
      const payload = JSON.parse(event.data) as LogStreamEvent;
      switch (payload.type) {
        case 'append':
          opts.onAppend?.(payload.entry_index, payload.entry);
          break;
        case 'replace':
          opts.onReplace?.(payload.entry_index, payload.entry);
          break;
        case 'finished':
          finished = true;
          opts.onFinished?.();
          break;
        default:
          break;
      }
    } catch (err) {
      opts.onError?.(err);
    }
  });

  ws.addEventListener('error', (err) => {
    connected = false;
    sawError = true;
    opts.onError?.(err);
  });

  ws.addEventListener('close', () => {
    connected = false;
    if (!finished && !closedByClient && !sawError) {
      opts.onError?.(new Error('Log stream closed unexpectedly'));
    }
  });

  return {
    close() {
      closedByClient = true;
      ws.close();
    },
    isConnected() {
      return connected;
    },
  };
}
