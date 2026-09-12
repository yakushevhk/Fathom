import { useRef, useState, useEffect } from "react";
import { Play, Pause, Volume2, RotateCcw } from "lucide-react";
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

  // Pseudorandom static waveform heights for aesthetic display
  const bars = [35, 60, 45, 90, 75, 40, 65, 80, 50, 95, 70, 40, 85, 60, 45, 70, 90, 55, 35, 65];

  const togglePlay = () => {
    triggerHaptic("tap");
    if (!audioRef.current) return;
    if (playing) {
      audioRef.current.pause();
      setPlaying(false);
    } else {
      void audioRef.current.play();
      setPlaying(true);
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

  const formatSec = (sec: number) => {
    const mins = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${mins}:${s < 10 ? "0" : ""}${s}`;
  };

  return (
    <div className="my-1 flex w-full max-w-[340px] flex-col gap-2 rounded-xl border border-hairline/60 bg-panel p-3 shadow-md">
      <audio
        ref={audioRef}
        src={src}
        preload="metadata"
        onTimeUpdate={() => {
          if (!audioRef.current) return;
          setCurrentTime(audioRef.current.currentTime);
          setProgress((audioRef.current.currentTime / (audioRef.current.duration || 1)) * 100);
        }}
        onLoadedMetadata={() => {
          if (audioRef.current) setDuration(audioRef.current.duration);
        }}
        onEnded={() => {
          setPlaying(false);
          setProgress(0);
        }}
      />

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-[12px] font-semibold text-ink truncate pr-2">
          <Volume2 size={15} className="text-accent shrink-0" />
          <span className="truncate">{fileName}</span>
        </div>
        <span className="font-mono text-[11px] text-ink-secondary">
          {formatSec(currentTime)} / {formatSec(duration || 0)}
        </span>
      </div>

      {/* Waveform & Scrubber */}
      <div
        className="flex items-center gap-1 h-8 cursor-pointer py-1"
        onClick={(e) => {
          if (!audioRef.current || !duration) return;
          const rect = e.currentTarget.getBoundingClientRect();
          const clickX = e.clientX - rect.left;
          const pct = Math.max(0, Math.min(1, clickX / rect.width));
          audioRef.current.currentTime = pct * duration;
          setProgress(pct * 100);
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
          className="flex size-7 items-center justify-center rounded-full bg-accent text-white shadow hover:scale-105 active:scale-95 transition-transform"
        >
          {playing ? <Pause size={13} /> : <Play size={13} className="ml-0.5" />}
        </button>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={cycleSpeed}
            className="rounded bg-raised px-2 py-0.5 text-[11px] font-mono font-medium text-ink-secondary hover:text-ink transition-colors"
          >
            {playbackRate}x
          </button>
        </div>
      </div>
    </div>
  );
}
