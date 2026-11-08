// Browser playback for the gallery's audio intents.
export function audioEffects() {
  const sounds = new Map();
  const playing = new Map();
  let context;
  let master;
  let masterVolume = 1;

  function unlock() {
    try {
      if (!context) {
        context = new AudioContext({ sampleRate: 44100 });
        master = context.createGain();
        master.gain.value = masterVolume;
        master.connect(context.destination);
      }
      if (context.state !== "running") context.resume().catch(() => {});
    } catch {
      // An unavailable device is reported when a sound is requested.
    }
  }

  function stop(playback) {
    if (!playback.source) return;
    playback.offset += context.currentTime - playback.started;
    const source = playback.source;
    playback.source = undefined;
    source.stop();
  }

  function start(playback, ended) {
    if (!playback.buffer || playback.paused || playback.source) return;
    const source = context.createBufferSource();
    const gain = context.createGain();
    source.buffer = playback.buffer;
    source.loop = playback.loop;
    gain.gain.value = playback.volume;
    source.connect(gain);
    gain.connect(master);
    source.onended = () => {
      source.disconnect();
      gain.disconnect();
      if (playback.source === source) ended();
    };
    playback.source = source;
    playback.gain = gain;
    playback.started = context.currentTime;
    const offset = playback.loop ? playback.offset % source.buffer.duration : playback.offset;
    source.start(playback.started, offset);
  }

  function handle(req, notify) {
    const current = playing.get(req.id);
    switch (req.kind) {
      case "play": {
        const playback = { volume: req.volume, loop: req.loop, offset: 0 };
        const ended = (refused = false) => {
          if (playing.get(req.id) !== playback) return;
          playing.delete(req.id);
          notify(refused);
        };
        playing.set(req.id, playback);
        // Skip hover sounds before the first user gesture.
        if (!req.bytes || !context) { ended(true); break; }
        let buffer = sounds.get(req.sound);
        if (!buffer) {
          buffer = context.decodeAudioData(Uint8Array.from(req.bytes).buffer);
          sounds.set(req.sound, buffer);
          buffer.catch(() => {
            if (sounds.get(req.sound) === buffer) sounds.delete(req.sound);
          });
        }
        playback.ended = ended;
        buffer.then(decoded => {
          if (playing.get(req.id) !== playback) return;
          playback.buffer = decoded;
          start(playback, ended);
        }).catch(() => ended(true));
        break;
      }
      case "stop":
        if (current) stop(current);
        playing.delete(req.id);
        break;
      case "pause":
        if (current) {
          current.paused = true;
          stop(current);
        }
        break;
      case "resume":
        if (current) {
          current.paused = false;
          start(current, current.ended);
        }
        break;
      case "volume":
        if (current) {
          current.volume = req.volume;
          if (current.gain) current.gain.gain.value = req.volume;
        }
        break;
      case "master":
        masterVolume = req.volume;
        if (master) master.gain.value = masterVolume;
        break;
      case "unload":
        sounds.delete(req.sound);
        break;
    }
  }

  return { handle, unlock };
}
