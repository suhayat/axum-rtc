/**
 * mediasoup-client integration
 */
import { Device } from 'mediasoup-client';
import type { Transport, Producer, Consumer, RtpCapabilities } from 'mediasoup-client/types';
import { SignalingClient } from './signaling';

export type RemoteStream = {
  peerId: string;
  stream: MediaStream;
  consumers: Map<string, Consumer>;
};

export type MediasoupCallbacks = {
  onRemoteStream: (peerId: string, stream: MediaStream) => void;
  onRemoteStreamRemoved: (peerId: string) => void;
  onDisconnected: () => void;
};

export class MediasoupManager {
  private signaling: SignalingClient;
  private device: Device | null = null;
  private sendTransport: Transport | null = null;
  private recvTransport: Transport | null = null;
  private audioProducer: Producer | null = null;
  private videoProducer: Producer | null = null;
  private consumers = new Map<string, Consumer>();
  private remoteStreams = new Map<string, MediaStream>();
  private callbacks: MediasoupCallbacks;
  private localStream: MediaStream | null = null;

  constructor(callbacks: MediasoupCallbacks) {
    this.signaling = new SignalingClient();
    this.callbacks = callbacks;
  }

  async join(roomId: string): Promise<MediaStream> {
    // 1. Connect signaling
    await this.signaling.connect(roomId);

    // Set up notification handlers
    this.signaling.setOnNotification((method, data) => {
      this.handleNotification(method, data);
    });

    this.signaling.setOnClose(() => {
      this.callbacks.onDisconnected();
    });

    // 2. Get Router RTP Capabilities and load device
    const routerRtpCapabilities: RtpCapabilities = await this.signaling.request('getRouterRtpCapabilities');
    this.device = new Device();
    await this.device.load({ routerRtpCapabilities });

    // 3. Create send transport
    await this.createSendTransport();

    // 4. Create recv transport
    await this.createRecvTransport();

    // 5. Get local media
    this.localStream = await navigator.mediaDevices.getUserMedia({
      audio: true,
      video: {
        width: { ideal: 1280 },
        height: { ideal: 720 },
        frameRate: { ideal: 30 },
      },
    });

    // 6. Produce audio and video
    const audioTrack = this.localStream.getAudioTracks()[0];
    const videoTrack = this.localStream.getVideoTracks()[0];

    if (audioTrack) {
      this.audioProducer = await this.sendTransport!.produce({ track: audioTrack });
    }
    if (videoTrack) {
      this.videoProducer = await this.sendTransport!.produce({ 
        track: videoTrack,
        encodings: [
          {
            maxBitrate: 500_000,
            scaleResolutionDownBy: 2,
          }
        ],
        codecOptions: {
          videoGoogleStartBitrate: 100,
          videoGoogleMinBitrate: 50,
          videoGoogleMaxBitrate: 500,
        } 
      });
    }

    // 7. Get existing producers and consume them
    const existingProducers = await this.signaling.request('getProducers');
    if (Array.isArray(existingProducers)) {
      for (const { peerId, producerId } of existingProducers) {
        await this.consumeProducer(peerId, producerId);
      }
    }

    return this.localStream;
  }

  private async createSendTransport() {
    const transportData = await this.signaling.request('createWebRtcTransport', {
      direction: 'send',
    });

    this.sendTransport = this.device!.createSendTransport({
      id: transportData.id,
      iceParameters: transportData.iceParameters,
      iceCandidates: transportData.iceCandidates,
      dtlsParameters: transportData.dtlsParameters,
    });

    this.sendTransport.on('connect', async ({ dtlsParameters }, callback, errback) => {
      try {
        await this.signaling.request('connectTransport', {
          transportId: this.sendTransport!.id,
          dtlsParameters,
        });
        callback();
      } catch (e: any) {
        errback(e);
      }
    });

    this.sendTransport.on('produce', async ({ kind, rtpParameters }, callback, errback) => {
      try {
        const { producerId } = await this.signaling.request('produce', {
          kind,
          rtpParameters,
        });
        callback({ id: producerId });
      } catch (e: any) {
        errback(e);
      }
    });
  }

  private async createRecvTransport() {
    const transportData = await this.signaling.request('createWebRtcTransport', {
      direction: 'recv',
    });

    this.recvTransport = this.device!.createRecvTransport({
      id: transportData.id,
      iceParameters: transportData.iceParameters,
      iceCandidates: transportData.iceCandidates,
      dtlsParameters: transportData.dtlsParameters,
    });

    this.recvTransport.on('connect', async ({ dtlsParameters }, callback, errback) => {
      try {
        await this.signaling.request('connectTransport', {
          transportId: this.recvTransport!.id,
          dtlsParameters,
        });
        callback();
      } catch (e: any) {
        errback(e);
      }
    });
  }

  private async consumeProducer(peerId: string, producerId: string) {
    if (!this.device || !this.recvTransport) return;

    try {
      const data = await this.signaling.request('consume', {
        producerId,
        rtpCapabilities: this.device.recvRtpCapabilities,
      });

      const consumer = await this.recvTransport.consume({
        id: data.consumerId,
        producerId: data.producerId,
        kind: data.kind,
        rtpParameters: data.rtpParameters,
      });

      this.consumers.set(data.consumerId, consumer);

      // Get or create the remote stream for this peer
      let oldStream = this.remoteStreams.get(peerId);
      let stream: MediaStream;

      if (!oldStream) {
        stream = new MediaStream();
      } else {
        // Create a new MediaStream with existing tracks to force reactivity
        stream = new MediaStream(oldStream.getTracks());
      }

      stream.addTrack(consumer.track);
      this.remoteStreams.set(peerId, stream);

      // Resume the consumer
      await this.signaling.request('resumeConsumer', { consumerId: data.consumerId });

      // Notify the UI
      this.callbacks.onRemoteStream(peerId, stream);
    } catch (e) {
      console.error('[Mediasoup] Failed to consume:', e);
    }
  }

  private async handleNotification(method: string, data: any) {
    switch (method) {
      case 'newProducer': {
        const { peerId, producerId } = data;
        await this.consumeProducer(peerId, producerId);
        break;
      }
      case 'peerClosed': {
        const { peerId } = data;
        // Remove all consumers for this peer
        this.remoteStreams.delete(peerId);
        this.callbacks.onRemoteStreamRemoved(peerId);
        break;
      }
    }
  }

  toggleAudio(): boolean {
    if (this.audioProducer) {
      if (this.audioProducer.paused) {
        this.audioProducer.resume();
        return true; // unmuted
      } else {
        this.audioProducer.pause();
        return false; // muted
      }
    }
    return false;
  }

  toggleVideo(): boolean {
    if (this.videoProducer) {
      if (this.videoProducer.paused) {
        this.videoProducer.resume();
        // Re-enable the video track
        if (this.localStream) {
          const track = this.localStream.getVideoTracks()[0];
          if (track) track.enabled = true;
        }
        return true; // video on
      } else {
        this.videoProducer.pause();
        // Disable the video track
        if (this.localStream) {
          const track = this.localStream.getVideoTracks()[0];
          if (track) track.enabled = false;
        }
        return false; // video off
      }
    }
    return false;
  }

  leave() {
    // Close all consumers
    for (const [, consumer] of this.consumers) {
      consumer.close();
    }
    this.consumers.clear();
    this.remoteStreams.clear();

    // Close producers
    this.audioProducer?.close();
    this.videoProducer?.close();

    // Close transports
    this.sendTransport?.close();
    this.recvTransport?.close();

    // Stop local tracks
    if (this.localStream) {
      this.localStream.getTracks().forEach((t) => t.stop());
      this.localStream = null;
    }

    // Disconnect signaling
    this.signaling.disconnect();
  }
}
