import NiceModal from '@ebay/nice-modal-react';

import App from '@app/App';

export default function RootApp() {
  return <App ModalProvider={NiceModal.Provider} />;
}
