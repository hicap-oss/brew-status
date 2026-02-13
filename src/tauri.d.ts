/* eslint-disable @typescript-eslint/no-explicit-any */
interface Window {
  __TAURI__: {
    core: {
      invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
    };
    event: {
      listen: (event: string, handler: (event: any) => void) => Promise<() => void>;
    };
    webviewWindow: {
      getCurrentWebviewWindow: () => any;
      WebviewWindow: any;
    };
  };
}
