import { useCallback, useEffect, useRef, useState } from "react";

export function useVideoStream({ apiBase, selectedPrinterId }) {
  const videoRef = useRef(null);
  const [videoError, setVideoError] = useState("");
  const [videoKey, setVideoKey] = useState(0);
  const [showVideoMenu, setShowVideoMenu] = useState(false);
  const [streamEnabled, setStreamEnabled] = useState(() => {
    if (typeof document === "undefined") {
      return true;
    }
    return !document.hidden;
  });
  const videoMenuTimeoutRef = useRef(null);

  const cmafWsUrl = selectedPrinterId
    ? `${apiBase.replace(/^http/, "ws")}/api/printers/${selectedPrinterId}/video/cmaf`
    : "";

  const reloadVideo = useCallback(() => {
    setVideoKey((value) => value + 1);
  }, []);

  useEffect(() => {
    reloadVideo();
  }, [reloadVideo, selectedPrinterId]);

  useEffect(() => {
    if (typeof document === "undefined") {
      return () => {};
    }

    const handleVisibilityChange = () => {
      const isVisible = !document.hidden;
      setStreamEnabled(isVisible);
      const video = videoRef.current;
      if (!isVisible && video) {
        video.pause();
      }
      if (isVisible) {
        reloadVideo();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    window.addEventListener("pageshow", handleVisibilityChange);
    window.addEventListener("pagehide", handleVisibilityChange);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      window.removeEventListener("pageshow", handleVisibilityChange);
      window.removeEventListener("pagehide", handleVisibilityChange);
    };
  }, [reloadVideo]);

  const revealVideoMenu = useCallback(() => {
    setShowVideoMenu(true);
    if (videoMenuTimeoutRef.current) {
      clearTimeout(videoMenuTimeoutRef.current);
    }
    videoMenuTimeoutRef.current = setTimeout(() => {
      setShowVideoMenu(false);
    }, 1800);
  }, []);

  const handleVideoPointerDown = useCallback(
    (event) => {
      if (event.pointerType === "touch") {
        revealVideoMenu();
      }
    },
    [revealVideoMenu],
  );

  useEffect(() => {
    return () => {
      if (videoMenuTimeoutRef.current) {
        clearTimeout(videoMenuTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    setVideoError("");

    if (!streamEnabled) {
      video.pause();
      video.removeAttribute("src");
      video.load();
      return () => {};
    }

    if (!cmafWsUrl) {
      setVideoError("Select a printer to load video");
      video.removeAttribute("src");
      video.load();
      return () => {};
    }

    const onVideoError = () => {
      setVideoError("Video element error");
    };
    video.addEventListener("error", onVideoError);

    const ensureAutoplay = () => {
      video.muted = true;
      video.defaultMuted = true;
      video.autoplay = true;
      video.playsInline = true;
      video.setAttribute("playsinline", "");
      video.setAttribute("muted", "");
      video.setAttribute("autoplay", "");
    };

    ensureAutoplay();

    const isSafari =
      typeof navigator !== "undefined" && /Apple/.test(navigator.vendor);

    const waitForEvent = (target, name) =>
      new Promise((resolve) => {
        target.addEventListener(name, resolve, { once: true });
      });

    const parseCodec = (value) => {
      if (!value) {
        return "";
      }
      const match = value.match(/codecs\s*=\s*"?([^";]+)"?/i);
      if (match?.[1]) {
        return match[1].trim();
      }
      if (value.startsWith("avc1.")) {
        return value.trim();
      }
      return "";
    };

    const setupMse = async (MSEClass, registerCleanup) => {
      const mediaSource = new MSEClass();
      const objectUrl = URL.createObjectURL(mediaSource);
      let sourceBuffer = null;
      let closed = false;
      let ws = null;
      let pendingChunks = [];
      let codecReadyResolve = null;
      let codecReadyReject = null;
      let codecPromise = null;
      let wsClosed = false;
      let liveEdgeTimer = null;
      let liveEdgeStarted = false;

      const attachMediaSource = () => {
        if (isSafari) {
          video.disableRemotePlayback = true;
          video.setAttribute("disableremoteplayback", "");
        }
        video.src = objectUrl;
      };

      const teardown = () => {
        closed = true;
        if (liveEdgeTimer) {
          clearInterval(liveEdgeTimer);
          liveEdgeTimer = null;
        }
        if (ws) {
          ws.close();
        }
        if (mediaSource.readyState === "open") {
          try {
            mediaSource.endOfStream();
          } catch (err) {
            // ignore
          }
        }
        URL.revokeObjectURL(objectUrl);
        video.disableRemotePlayback = false;
        video.removeAttribute("disableremoteplayback");
        video.removeAttribute("src");
        video.load();
      };
      registerCleanup(teardown);

      try {
        if (!cmafWsUrl) {
          throw new Error("CMAF websocket unavailable");
        }

        codecPromise = new Promise((resolve, reject) => {
          codecReadyResolve = resolve;
          codecReadyReject = reject;
        });
        ws = new WebSocket(cmafWsUrl);
        ws.binaryType = "arraybuffer";
        ws.onmessage = (event) => {
          if (typeof event.data === "string") {
            const value = event.data.trim();
            if (value.startsWith("codec:")) {
              const parsed = parseCodec(value.slice("codec:".length).trim());
              if (parsed && codecReadyResolve) {
                codecReadyResolve(parsed);
                codecReadyResolve = null;
                codecReadyReject = null;
              }
            }
            return;
          }
          if (event.data instanceof ArrayBuffer) {
            pendingChunks.push(new Uint8Array(event.data));
          }
        };
        ws.onerror = () => {
          if (codecReadyReject) {
            codecReadyReject(new Error("CMAF websocket failed"));
            codecReadyResolve = null;
            codecReadyReject = null;
          }
        };
        ws.onclose = () => {
          wsClosed = true;
          if (codecReadyReject) {
            codecReadyReject(new Error("CMAF websocket closed"));
            codecReadyResolve = null;
            codecReadyReject = null;
          }
        };

        const codec = await Promise.race([
          codecPromise,
          new Promise((_, reject) =>
            setTimeout(
              () => reject(new Error("CMAF websocket codec timeout")),
              5000,
            ),
          ),
        ]).catch((err) => {
          throw err;
        });
        const resolvedCodec = codec || "avc1.42E01E";

        const mime = `video/mp4; codecs="${resolvedCodec}"`;
        if (typeof MSEClass.isTypeSupported === "function") {
          if (!MSEClass.isTypeSupported(mime)) {
            throw new Error(`MSE unsupported codec: ${resolvedCodec}`);
          }
        }

        attachMediaSource();

        if (mediaSource.readyState !== "open") {
          await waitForEvent(mediaSource, "sourceopen");
        }
        sourceBuffer = mediaSource.addSourceBuffer(mime);
        try {
          sourceBuffer.mode = "sequence";
        } catch (err) {
          // Ignore if mode is not supported.
        }
        const waitForUpdate = () => waitForEvent(sourceBuffer, "updateend");

        const trimBuffer = () => {
          if (!video.buffered || video.buffered.length === 0) {
            return;
          }
          const end = video.buffered.end(video.buffered.length - 1);
          const maxAhead = 3.0;
          const keepBehind = 2.0;
          if (end - video.currentTime > maxAhead) {
            const removeEnd = Math.max(0, video.currentTime - keepBehind);
            if (removeEnd > 0 && !sourceBuffer.updating) {
              sourceBuffer.remove(0, removeEnd);
            }
          }
        };

        const appendWithBackpressure = async (chunk) => {
          if (closed) {
            return;
          }
          while (sourceBuffer.updating) {
            await waitForUpdate();
          }
          sourceBuffer.appendBuffer(chunk);
          await waitForUpdate();
          trimBuffer();
        };

        const getBufferedRange = () => {
          if (!video.buffered || video.buffered.length === 0) {
            return null;
          }
          const tolerance = 0.1;
          for (let index = 0; index < video.buffered.length; index += 1) {
            const start = video.buffered.start(index);
            const end = video.buffered.end(index);
            if (
              Number.isFinite(start) &&
              Number.isFinite(end) &&
              video.currentTime >= start - tolerance &&
              video.currentTime <= end + tolerance
            ) {
              return { start, end };
            }
          }

          const index = video.buffered.length - 1;
          const start = video.buffered.start(index);
          const end = video.buffered.end(index);
          if (!Number.isFinite(start) || !Number.isFinite(end)) {
            return null;
          }
          return { start, end };
        };

        const getLiveEdge = () => {
          const range = getBufferedRange();
          return range ? range.end : null;
        };

        const getBufferedAhead = () => {
          const liveEdge = getLiveEdge();
          if (liveEdge == null) {
            return 0;
          }
          return Math.max(0, liveEdge - video.currentTime);
        };

        const seekTo = (time) => {
          if (typeof video.fastSeek === "function") {
            video.fastSeek(time);
          } else {
            video.currentTime = time;
          }
        };

        const syncToLiveEdge = (force = false) => {
          const range = getBufferedRange();
          if (!range) {
            return;
          }
          const targetLatency = 0.45;
          const hardJumpLag = 1.8;
          const lag = range.end - video.currentTime;
          if ((force || lag > hardJumpLag) && !video.seeking) {
            seekTo(Math.max(range.start, range.end - targetLatency));
            if (video.playbackRate !== 1) {
              video.playbackRate = 1;
            }
            return;
          }

          let playbackRate = 1;
          if (lag > targetLatency + 0.6) {
            playbackRate = 1.08;
          } else if (lag > targetLatency + 0.25) {
            playbackRate = 1.04;
          }

          if (Math.abs(video.playbackRate - playbackRate) > 0.01) {
            video.playbackRate = playbackRate;
          }
        };

        const startLiveEdgeTicker = () => {
          if (liveEdgeStarted) {
            return;
          }
          liveEdgeStarted = true;
          liveEdgeTimer = setInterval(() => {
            if (closed) {
              return;
            }
            syncToLiveEdge(false);
            if (video.paused) {
              if (getBufferedAhead() > 0.1) {
                video.play().catch(() => {});
              }
            }
          }, 500);
        };

        const flushPending = async () => {
          while (pendingChunks.length > 0 && !closed) {
            const chunk = pendingChunks.shift();
            if (chunk) {
              await appendWithBackpressure(chunk);
            }
          }
        };

        let started = false;

        const noteStarted = () => {
          if (started) {
            return;
          }
          if (getBufferedAhead() < 0.25) {
            return;
          }
          started = true;
          syncToLiveEdge(true);
          video.play().catch(() => {});
          startLiveEdgeTicker();
        };

        await flushPending();
        noteStarted();
        while (!closed) {
          if (pendingChunks.length === 0) {
            if (wsClosed) {
              break;
            }
            await new Promise((resolve) => setTimeout(resolve, 50));
            continue;
          }
          await flushPending();
          await noteStarted();
        }
      } catch (err) {
        teardown();
        throw err;
      }

      return teardown;
    };

    let cleanup = () => {};
    let cancelled = false;

    const start = async () => {
      const MSEClass =
        typeof window !== "undefined"
          ? (window.ManagedMediaSource ?? window.MediaSource)
          : null;

      if (cmafWsUrl) {
        try {
          cleanup = await setupMse(MSEClass, (value) => {
            cleanup = value;
          });
          if (cancelled) {
            cleanup();
          }
          return;
        } catch (err) {
          if (cancelled) {
            return;
          }
          console.warn("CMAF MSE failed", err);
        }
      }
      if (!cancelled) {
        setVideoError("MSE is not supported in this browser");
      }
    };

    start();

    return () => {
      cancelled = true;
      cleanup();
      video.removeEventListener("error", onVideoError);
    };
  }, [cmafWsUrl, videoKey, streamEnabled]);

  return {
    videoRef,
    videoError,
    videoKey,
    showVideoMenu,
    handleVideoPointerDown,
    reloadVideo,
  };
}
