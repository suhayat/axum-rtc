/**
 * WebSocket signaling client for mediasoup
 */

type RequestResolver = {
  resolve: (data: any) => void;
  reject: (error: Error) => void;
};

export class SignalingClient {
  private ws: WebSocket | null = null;
  private requestId = 0;
  private pendingRequests = new Map<number, RequestResolver>();
  private onNotification: ((method: string, data: any) => void) | null = null;
  private onClose: (() => void) | null = null;

  constructor() { }

  connect(roomId: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = `${protocol}//${window.location.host}/ws?room=${encodeURIComponent(roomId)}`;

      this.ws = new WebSocket(wsUrl);

      this.ws.onopen = () => {
        console.log('[Signaling] Connected');
        resolve();
      };

      this.ws.onerror = (e) => {
        console.error('[Signaling] Error:', e);
        reject(new Error('WebSocket connection failed'));
      };

      this.ws.onclose = () => {
        console.log('[Signaling] Disconnected');
        // Reject all pending requests
        for (const [, { reject }] of this.pendingRequests) {
          reject(new Error('Connection closed'));
        }
        this.pendingRequests.clear();
        this.onClose?.();
      };

      this.ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);

          // It's a response to a request
          if (msg.id !== undefined && msg.id !== null) {
            const pending = this.pendingRequests.get(msg.id);
            if (pending) {
              this.pendingRequests.delete(msg.id);
              if (msg.ok) {
                pending.resolve(msg.data);
              } else {
                pending.reject(new Error(msg.error || 'Unknown error'));
              }
            }
          }
          // It's a server notification
          else if (msg.method) {
            this.onNotification?.(msg.method, msg.data);
          }
        } catch (e) {
          console.error('[Signaling] Failed to parse message:', e);
        }
      };
    });
  }

  setOnNotification(handler: (method: string, data: any) => void) {
    this.onNotification = handler;
  }

  setOnClose(handler: () => void) {
    this.onClose = handler;
  }

  async request(method: string, data: any = {}): Promise<any> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket not connected');
    }

    const id = ++this.requestId;

    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });

      this.ws!.send(JSON.stringify({
        id,
        method,
        data,
      }));

      // Timeout after 10 seconds
      setTimeout(() => {
        if (this.pendingRequests.has(id)) {
          this.pendingRequests.delete(id);
          reject(new Error(`Request ${method} timed out`));
        }
      }, 10000);
    });
  }

  disconnect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}
