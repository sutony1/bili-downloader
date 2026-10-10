(() => {
  // Pause the webpage preview without changing the downloaded audio stream.
  const pause = root => {
    root.querySelectorAll('video, audio').forEach(media => {
      media.pause();
    });
    root.querySelectorAll('iframe').forEach(frame => {
      try { if (frame.contentDocument) pause(frame.contentDocument); } catch {}
    });
  };
  pause(document);
})();
