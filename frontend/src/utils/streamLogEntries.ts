import type { LogStreamEvent, PatchType } from 'shared/types';
import { createWebSocket } from '@/lib/api';

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

const normalizeEntryIndex = (value: bigint | number) =>
  typeof value === 'bigint' ? value : BigInt(value);

export function streamLogEntries(
  url: string,
  opts: StreamOptions = {}
): StreamController {
  const ws = createWebSocket(url);
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
          opts.onAppend?.(
            normalizeEntryIndex(payload.entry_index),
            payload.entry
          );
          break;
        case 'replace':
          opts.onReplace?.(
            normalizeEntryIndex(payload.entry_index),
            payload.entry
          );
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

  ws.addEventListener('close', (event) => {
    connected = false;
    if (!finished && !closedByClient && !sawError) {
      const err = Object.assign(
        new Error(
          `Log stream closed (code=${event.code}${
            event.reason ? `, reason=${event.reason}` : ''
          })`
        ),
        {
          code: event.code,
          reason: event.reason,
          wasClean: event.wasClean,
        }
      );
      opts.onError?.(err);
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
