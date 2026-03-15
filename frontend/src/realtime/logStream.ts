import { streamLogEntries } from '@/utils/streamLogEntries';

export type LogStreamOptions = Parameters<typeof streamLogEntries>[1];
export type LogStreamController = ReturnType<typeof streamLogEntries>;

export const openLogStream = streamLogEntries;
