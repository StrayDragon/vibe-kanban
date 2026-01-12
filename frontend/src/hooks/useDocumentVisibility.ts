import { useEffect, useState } from 'react';

export function useDocumentVisibility() {
  const [isVisible, setIsVisible] = useState(() =>
    typeof document === 'undefined'
      ? true
      : document.visibilityState === 'visible'
  );

  useEffect(() => {
    const handleVisibilityChange = () => {
      setIsVisible(document.visibilityState === 'visible');
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  return isVisible;
}
