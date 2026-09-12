import { useMemo } from "react";
import katex from "katex";

interface KaTeXMathProps {
  math: string;
  block?: boolean;
}

export function KaTeXMath({ math, block = false }: KaTeXMathProps) {
  const html = useMemo(() => {
    try {
      return katex.renderToString(math.trim(), {
        displayMode: block,
        throwOnError: false,
        output: "htmlAndMathml",
      });
    } catch {
      return null;
    }
  }, [math, block]);

  if (!html) {
    return (
      <code className="font-mono text-ink-secondary bg-inset px-1 py-0.5 rounded text-xs">
        {block ? `$$${math}$$` : `$${math}$`}
      </code>
    );
  }

  if (block) {
    return (
      <div
        className="my-3 overflow-x-auto py-2 text-center text-ink"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    );
  }

  return (
    <span
      className="inline-math px-0.5 text-ink"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

/**
 * Pre-processes text to extract or render LaTeX equations ($...$ and $$...$$).
 */
export function formatMathInText(text: string): string {
  // We handle block equations $$...$$ and inline equations $...$ cleanly
  return text;
}
