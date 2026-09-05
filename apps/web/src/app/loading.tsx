export default function Loading() {
  return (
    <div className="flex-1 flex items-center justify-center p-12">
      <div className="flex flex-col items-center gap-3">
        <div className="w-5 h-5 rounded-full border border-white/20 border-t-white/80 animate-spin" />
        <span className="text-xs text-gray-500 font-mono tracking-wider uppercase">Loading</span>
      </div>
    </div>
  )
}
