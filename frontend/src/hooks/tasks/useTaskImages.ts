import { useQuery } from '@tanstack/react-query';
import { imagesApi } from '@/lib/api';
import type { ImageResponse } from 'shared/types';
import { imageKeys } from '@/query-keys/imageKeys';

export function useTaskImages(taskId?: string) {
  return useQuery<ImageResponse[]>({
    queryKey: imageKeys.taskImages(taskId),
    queryFn: () => imagesApi.getTaskImages(taskId!),
    enabled: !!taskId,
  });
}
