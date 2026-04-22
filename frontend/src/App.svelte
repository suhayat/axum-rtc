<script lang="ts">
  import { MediasoupManager } from './lib/mediasoup';

  // State
  let joined = $state(false);
  let roomId = $state('');
  let localStream: MediaStream | null = $state(null);
  let remoteStreams: Map<string, MediaStream> = $state(new Map());
  let audioEnabled = $state(true);
  let videoEnabled = $state(true);
  let manager: MediasoupManager | null = $state(null);
  let joining = $state(false);
  let error = $state('');

  // Track remote video elements
  let localVideoEl: HTMLVideoElement = $state();

  function getRemoteEntries(): [string, MediaStream][] {
    return Array.from(remoteStreams.entries());
  }

  async function joinRoom() {
    if (!roomId.trim()) {
      error = 'Please enter a room name';
      return;
    }

    joining = true;
    error = '';

    try {
      manager = new MediasoupManager({
        onRemoteStream: (peerId: string, stream: MediaStream) => {
          console.log(`[App] Received remote stream from ${peerId}`, stream.getTracks());
          remoteStreams = new Map(remoteStreams).set(peerId, stream);
        },
        onRemoteStreamRemoved: (peerId: string) => {
          const newMap = new Map(remoteStreams);
          newMap.delete(peerId);
          remoteStreams = newMap;
        },
        onDisconnected: () => {
          leaveRoom();
        },
      });

      const stream = await manager.join(roomId.trim());
      localStream = stream;
      joined = true;
    } catch (e: any) {
      console.error('Failed to join:', e);
      error = e.message || 'Failed to join room';
      manager = null;
    } finally {
      joining = false;
    }
  }

  function leaveRoom() {
    manager?.leave();
    manager = null;
    localStream = null;
    remoteStreams = new Map();
    joined = false;
    audioEnabled = true;
    videoEnabled = true;
  }

  function toggleAudio() {
    if (manager) {
      audioEnabled = manager.toggleAudio();
    }
  }

  function toggleVideo() {
    if (manager) {
      videoEnabled = manager.toggleVideo();
    }
  }

  function setVideoSrc(el: HTMLVideoElement, stream: MediaStream | null) {
    if (el && stream) {
      if (el.srcObject !== stream) {
        el.srcObject = stream;
      }
    }
    return {
      update(newStream: MediaStream | null) {
        if (el && newStream && el.srcObject !== newStream) {
          el.srcObject = newStream;
        }
      }
    };
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !joined) {
      joinRoom();
    }
  }

  // Count total participants
  function participantCount(): number {
    return 1 + remoteStreams.size;
  }
</script>

<svelte:head>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet" />
</svelte:head>

{#if !joined}
  <!-- JOIN SCREEN -->
  <div class="join-screen">
    <div class="join-card">
      <div class="logo">
        <div class="logo-icon">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="23 7 16 12 23 17 23 7" />
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
          </svg>
        </div>
        <h1>Axum<span>RTC</span></h1>
        <p class="tagline">Video Conference</p>
      </div>

      <div class="join-form">
        <div class="input-group">
          <label for="room-input">Room Name</label>
          <input
            id="room-input"
            type="text"
            bind:value={roomId}
            onkeydown={handleKeydown}
            placeholder="Enter room name..."
            disabled={joining}
          />
        </div>

        {#if error}
          <div class="error-msg">{error}</div>
        {/if}

        <button
          class="join-btn"
          onclick={joinRoom}
          disabled={joining || !roomId.trim()}
        >
          {#if joining}
            <span class="spinner"></span>
            Connecting...
          {:else}
            Join Room
          {/if}
        </button>
      </div>

      <p class="hint">Share the room name with others to start a video call</p>
    </div>
  </div>
{:else}
  <!-- CONFERENCE SCREEN -->
  <div class="conference">
    <header class="top-bar">
      <div class="room-info">
        <div class="room-dot"></div>
        <span class="room-name">{roomId}</span>
        <span class="participant-count">{participantCount()} participant{participantCount() > 1 ? 's' : ''}</span>
      </div>
    </header>

    <div class="video-grid" class:single={remoteStreams.size === 0} class:duo={remoteStreams.size === 1} class:multi={remoteStreams.size > 1}>
      <!-- Local Video -->
      <div class="video-tile local">
        <video
          bind:this={localVideoEl}
          autoplay
          muted
          playsinline
          use:setVideoSrc={localStream}
        ></video>
        <div class="video-label">
          <span>You</span>
          {#if !audioEnabled}
            <span class="muted-icon" title="Muted">🔇</span>
          {/if}
          {#if !videoEnabled}
            <span class="cam-off-icon" title="Camera off">📷</span>
          {/if}
        </div>
        {#if !videoEnabled}
          <div class="video-off-overlay">
            <div class="avatar">You</div>
          </div>
        {/if}
      </div>

      <!-- Remote Videos -->
      {#each getRemoteEntries() as [peerId, stream] (peerId)}
        <div class="video-tile remote">
          <video
            autoplay
            playsinline
            use:setVideoSrc={stream}
          ></video>
          <div class="video-label">
            <span>Peer {peerId.slice(0, 6)}</span>
          </div>
        </div>
      {/each}
    </div>

    <div class="controls-bar">
      <button
        class="control-btn"
        class:off={!audioEnabled}
        onclick={toggleAudio}
        title={audioEnabled ? 'Mute' : 'Unmute'}
      >
        {#if audioEnabled}
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
            <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
            <line x1="12" y1="19" x2="12" y2="23" />
            <line x1="8" y1="23" x2="16" y2="23" />
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="1" y1="1" x2="23" y2="23" />
            <path d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6" />
            <path d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2c0 .76-.13 1.49-.36 2.18" />
            <line x1="12" y1="19" x2="12" y2="23" />
            <line x1="8" y1="23" x2="16" y2="23" />
          </svg>
        {/if}
        <span>{audioEnabled ? 'Mic' : 'Mic Off'}</span>
      </button>

      <button
        class="control-btn"
        class:off={!videoEnabled}
        onclick={toggleVideo}
        title={videoEnabled ? 'Turn off camera' : 'Turn on camera'}
      >
        {#if videoEnabled}
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="23 7 16 12 23 17 23 7" />
            <rect x="1" y="5" width="15" height="14" rx="2" ry="2" />
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="1" y1="1" x2="23" y2="23" />
            <path d="M21 21H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h3m3-3h6l2 3h4a2 2 0 0 1 2 2v9.34" />
          </svg>
        {/if}
        <span>{videoEnabled ? 'Cam' : 'Cam Off'}</span>
      </button>

      <button
        class="control-btn leave"
        onclick={leaveRoom}
        title="Leave room"
      >
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M10.68 13.31a16 16 0 0 0 3.41 2.6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7 2 2 0 0 1 1.72 2v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.42 19.42 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91" />
          <line x1="23" y1="1" x2="1" y2="23" />
        </svg>
        <span>Leave</span>
      </button>
    </div>
  </div>
{/if}

<style>
  /* ===== JOIN SCREEN ===== */
  .join-screen {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }

  .join-card {
    background: rgba(255, 255, 255, 0.05);
    backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 24px;
    padding: 3rem 2.5rem;
    width: 100%;
    max-width: 420px;
    text-align: center;
    animation: fadeUp 0.6s ease-out;
  }

  @keyframes fadeUp {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .logo {
    margin-bottom: 2.5rem;
  }

  .logo-icon {
    width: 56px;
    height: 56px;
    margin: 0 auto 1rem;
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    border-radius: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    box-shadow: 0 8px 32px rgba(99, 102, 241, 0.3);
  }

  .logo-icon svg {
    width: 28px;
    height: 28px;
  }

  .logo h1 {
    font-size: 1.75rem;
    font-weight: 700;
    color: #fff;
    margin: 0;
  }

  .logo h1 span {
    background: linear-gradient(135deg, #6366f1, #a78bfa);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .tagline {
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.9rem;
    margin-top: 0.25rem;
  }

  .join-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .input-group {
    text-align: left;
  }

  .input-group label {
    display: block;
    font-size: 0.8rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.6);
    margin-bottom: 0.5rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .input-group input {
    width: 100%;
    padding: 0.85rem 1rem;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    color: #fff;
    font-size: 1rem;
    font-family: 'Inter', sans-serif;
    outline: none;
    transition: all 0.3s ease;
    box-sizing: border-box;
  }

  .input-group input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  .input-group input:focus {
    border-color: #6366f1;
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.15);
    background: rgba(255, 255, 255, 0.1);
  }

  .error-msg {
    color: #f87171;
    font-size: 0.85rem;
    padding: 0.5rem 0.75rem;
    background: rgba(248, 113, 113, 0.1);
    border-radius: 8px;
    border: 1px solid rgba(248, 113, 113, 0.2);
  }

  .join-btn {
    width: 100%;
    padding: 0.85rem;
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    color: white;
    border: none;
    border-radius: 12px;
    font-size: 1rem;
    font-weight: 600;
    font-family: 'Inter', sans-serif;
    cursor: pointer;
    transition: all 0.3s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
  }

  .join-btn:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 8px 25px rgba(99, 102, 241, 0.4);
  }

  .join-btn:active:not(:disabled) {
    transform: translateY(0);
  }

  .join-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .hint {
    color: rgba(255, 255, 255, 0.35);
    font-size: 0.8rem;
    margin-top: 1.5rem;
  }

  /* ===== CONFERENCE SCREEN ===== */
  .conference {
    height: 100vh;
    display: flex;
    flex-direction: column;
    animation: fadeIn 0.4s ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .top-bar {
    padding: 1rem 1.5rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(10px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    z-index: 10;
  }

  .room-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .room-dot {
    width: 8px;
    height: 8px;
    background: #22c55e;
    border-radius: 50%;
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; box-shadow: 0 0 0 0 rgba(34, 197, 94, 0.5); }
    50% { opacity: 0.8; box-shadow: 0 0 0 6px rgba(34, 197, 94, 0); }
  }

  .room-name {
    font-weight: 600;
    color: #fff;
    font-size: 0.95rem;
  }

  .participant-count {
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.8rem;
    padding: 0.2rem 0.6rem;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 20px;
  }

  /* ===== VIDEO GRID ===== */
  .video-grid {
    flex: 1;
    display: grid;
    gap: 0.75rem;
    padding: 0.75rem;
    overflow: hidden;
  }

  .video-grid.single {
    grid-template-columns: 1fr;
    max-width: 900px;
    margin: 0 auto;
    width: 100%;
  }

  .video-grid.duo {
    grid-template-columns: 1fr 1fr;
  }

  .video-grid.multi {
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  }

  .video-tile {
    position: relative;
    border-radius: 16px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    min-height: 200px;
  }

  .video-tile video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .video-tile.local video {
    transform: scaleX(-1);
  }

  .video-label {
    position: absolute;
    bottom: 0.75rem;
    left: 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(8px);
    border-radius: 8px;
    font-size: 0.8rem;
    color: white;
    font-weight: 500;
  }

  .muted-icon, .cam-off-icon {
    font-size: 0.75rem;
  }

  .video-off-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, #1e1b4b, #312e81);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .avatar {
    width: 80px;
    height: 80px;
    border-radius: 50%;
    background: rgba(99, 102, 241, 0.3);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.2rem;
    font-weight: 600;
    color: white;
    border: 2px solid rgba(99, 102, 241, 0.5);
  }

  /* ===== CONTROLS BAR ===== */
  .controls-bar {
    padding: 1rem 1.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    background: rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(20px);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .control-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
    padding: 0.75rem 1.25rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    color: white;
    cursor: pointer;
    transition: all 0.2s ease;
    font-family: 'Inter', sans-serif;
    min-width: 72px;
  }

  .control-btn svg {
    width: 22px;
    height: 22px;
  }

  .control-btn span {
    font-size: 0.7rem;
    font-weight: 500;
    opacity: 0.7;
  }

  .control-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    transform: translateY(-2px);
  }

  .control-btn.off {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.3);
    color: #fca5a5;
  }

  .control-btn.off:hover {
    background: rgba(239, 68, 68, 0.3);
  }

  .control-btn.leave {
    background: rgba(239, 68, 68, 0.6);
    border-color: rgba(239, 68, 68, 0.4);
  }

  .control-btn.leave:hover {
    background: rgba(239, 68, 68, 0.8);
    box-shadow: 0 4px 20px rgba(239, 68, 68, 0.3);
  }

  /* ===== RESPONSIVE ===== */
  @media (max-width: 768px) {
    .video-grid.duo {
      grid-template-columns: 1fr;
    }

    .video-grid.multi {
      grid-template-columns: 1fr;
    }

    .controls-bar {
      padding: 0.75rem 1rem;
      gap: 0.5rem;
    }

    .control-btn {
      padding: 0.6rem 1rem;
      min-width: 60px;
    }

    .join-card {
      padding: 2rem 1.5rem;
    }
  }
</style>
