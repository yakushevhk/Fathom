import { useRef, useState, useEffect } from "react";
import { Play, Pause, Volume2 } from "lucide-react";
import { triggerHaptic } from "@/lib/haptics";

interface WaveformAudioPlayerProps {
  src: string;
  fileName: string;
}

export function WaveformAudioPlayer({ src, fileName }: WaveformAudioPlayerProps) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [duration, setDuration] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);
  const [playbackRate, setPlaybackRate] = useState<1 | 1.25 | 1.5 | 2>(1);

  // Reset state whenever src changes
  useEffect(() => {
    setPlaying(false);
    setProgress(0);
    setCurrentTime(0);
    setDuration(0);
    if (audioRef.current) {
      audioRef.current.playbackRate = playbackRate;
    }
  }, [src]);

  // Pseudorandom static waveform heights for aesthetic display
  const bars = [35, 60, 45, 90, 75, 40, 65, 80, 50, 95, 70, 40, 85, 60, 45, 70, 90, 55, 35, 65];

  const togglePlay = () => {
    triggerHaptic("tap");
    if (!audioRef.current) return;
    if (playing) {
      audioRef.current.pause();
      setPlaying(false);
    } else {
      const playPromise = audioRef.current.play();
      if (playPromise !== undefined) {
        playPromise
          .then(() => setPlaying(true))
          .catch((err) => {
            console.error("Audio playback error:", err);
            setPlaying(false);
          });
      } else {
        setPlaying(true);
      }
    }
  };

  const cycleSpeed = () => {
    triggerHaptic("selection");
    const nextRate: Record<number, 1 | 1.25 | 1.5 | 2> = {
      1: 1.25,
      1.25: 1.5,
      1.5: 2,
      2: 1,
    };
    const rate = nextRate[playbackRate];
    setPlaybackRate(rate);
    if (audioRef.current) {
      audioRef.current.playbackRate = rate;
    }
  };

  const seekFromPointer = (clientX: number, target: HTMLElement) => {
    if (!audioRef.current) return;
    const rect = target.getBoundingClientRect();
    if (rect.width <= 0) return;
    const clickX = clientX - rect.left;
    const pct = Math.max(0, Math.min(1, clickX / rect.width));
    const targetDuration = duration || audioRef.current.duration || 0;
    if (Number.isFinite(targetDuration) && targetDuration > 0) {
      const newTime = pct * targetDuration;
      audioRef.current.currentTime = newTime;
      setCurrentTime(newTime);
      setProgress(pct * 100);
    }
  };

  const formatSec = (sec: number) => {
    if (!Number.isFinite(sec) || isNaN(sec) || sec < 0) return "0:00";
    const mins = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${mins}:${s < 10 ? "0" : ""}${s}`;
  };

  return (
    <div className="my-1 flex w-full max-w-full sm:max-w-[340px] flex-col gap-2 rounded-xl border border-hairline/60 bg-panel p-3 shadow-md">
      <audio
        ref={audioRef}
        src={src}
        preload="metadata"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onTimeUpdate={() => {
          if (!audioRef.current) return;
          const current = audioRef.current.currentTime;
          const dur = audioRef.current.duration;
          setCurrentTime(current);
          if (Number.isFinite(dur) && dur > 0) {
            setProgress((current / dur) * 100);
          }
        }}
        onLoadedMetadata={() => {
          if (audioRef.current && Number.isFinite(audioRef.current.duration)) {
            setDuration(audioRef.current.duration);
            audioRef.current.playbackRate = playbackRate;
          }
        }}
        onEnded={() => {
          setPlaying(false);
          setProgress(0);
          setCurrentTime(0);
        }}
      />

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-[12px] font-semibold text-ink min-w-0 pr-2">
          <Volume2 size={15} className="text-accent shrink-0" />
          <span className="truncate">{fileName}</span>
        </div>
        <span className="font-mono text-[11px] text-ink-secondary shrink-0">
          {formatSec(currentTime)} / {formatSec(duration || 0)}
        </span>
      </div>

      {/* Waveform & Scrubber */}
      <div
        role="slider"
        aria-label={`Audio progress for ${fileName}`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress)}
        tabIndex={0}
        className="flex items-center gap-1 h-8 cursor-pointer py-1 select-none focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent rounded"
        onPointerDown={(e) => {
          e.preventDefault();
          seekFromPointer(e.clientX, e.currentTarget);
        }}
        onKeyDown={(e) => {
          if (!audioRef.current || !duration) return;
          if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
            e.preventDefault();
            const newTime = Math.max(0, audioRef.current.currentTime - 5);
            audioRef.current.currentTime = newTime;
            setCurrentTime(newTime);
            setProgress((newTime / duration) * 100);
          } else if (e.key === "ArrowRight" || e.key === "ArrowUp") {
            e.preventDefault();
            const newTime = Math.min(duration, audioRef.current.currentTime + 5);
            audioRef.current.currentTime = newTime;
            setCurrentTime(newTime);
            setProgress((newTime / duration) * 100);
          } else if (e.key === " " || e.key === "Enter") {
            e.preventDefault();
            togglePlay();
          }
        }}
      >
        {bars.map((height, i) => {
          const barPct = (i / bars.length) * 100;
          const active = barPct <= progress;
          return (
            <div
              key={i}
              className={`flex-1 rounded-full transition-all duration-150 ${
                active ? "bg-accent" : "bg-hairline hover:bg-hairline-strong"
              }`}
              style={{ height: `${height}%` }}
            />
          );
        })}
      </div>

      {/* Controls */}
      <div className="flex items-center justify-between pt-1 border-t border-hairline/40">
        <button
          type="button"
          onClick={togglePlay}
          aria-label={playing ? "Pause" : "Play"}
          className="flex size-7 items-center justify-center rounded-full bg-accent text-white shadow hover:scale-105 active:scale-95 transition-transform"
        >
          {playing ? <Pause size={13} /> : <Play size={13} className="ml-0.5" />}
        </button>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={cycleSpeed}
            aria-label={`Playback rate ${playbackRate}x`}
            className="rounded bg-raised px-2 py-0.5 text-[11px] font-mono font-medium text-ink-secondary hover:text-ink transition-colors"
          >
            {playbackRate}x
          </button>
        </div>
      </div>
    </div>
  );
}
