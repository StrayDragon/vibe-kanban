import { useUserSystem } from '@/components/ConfigProvider';
import { EditorType } from 'shared/types';

export function isEditorIntegrationEnabled(
  editorType: EditorType | null | undefined
): boolean {
  return editorType !== EditorType.NONE;
}

export function useEditorIntegrationEnabled(): boolean {
  const { config } = useUserSystem();
  return isEditorIntegrationEnabled(config?.editor?.editor_type);
}

