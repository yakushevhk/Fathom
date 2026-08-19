'use client'

import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeHighlight from 'rehype-highlight'
import type { Components } from 'react-markdown'

interface MarkdownProps {
  children: string
  className?: string
}

const components: Components = {
  pre({ children, ...props }) {
    return (
      <pre
        className="overflow-x-auto rounded-lg bg-black/50 border border-white/[0.06] p-3 my-2 text-[12px] leading-relaxed"
        {...props}
      >
        {children}
      </pre>
    )
  },
  code({ className, children, ...props }) {
    const isInline = !className
    if (isInline) {
      return (
        <code
          className="rounded px-1 py-0.5 text-blue-400 text-[12px]"
          style={{ backgroundColor: '#141414' }}
          {...props}
        >
          {children}
        </code>
      )
    }
    return (
      <code className={className} {...props}>
        {children}
      </code>
    )
  },
  table({ children, ...props }) {
    return (
      <div className="overflow-x-auto my-2">
        <table
          className="w-full border-collapse border border-white/[0.08] text-sm"
          {...props}
        >
          {children}
        </table>
      </div>
    )
  },
  th({ children, ...props }) {
    return (
      <th
        className="border border-white/[0.08] bg-white/[0.04] px-3 py-1.5 text-left text-xs font-semibold text-gray-300"
        {...props}
      >
        {children}
      </th>
    )
  },
  td({ children, ...props }) {
    return (
      <td
        className="border border-white/[0.08] px-3 py-1.5 text-xs text-gray-400"
        {...props}
      >
        {children}
      </td>
    )
  },
  a({ children, href, ...props }) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className="text-blue-400 hover:text-blue-300 underline underline-offset-2"
        {...props}
      >
        {children}
      </a>
    )
  },
  p({ children, ...props }) {
    return (
      <p className="my-1.5 last:mb-0" {...props}>
        {children}
      </p>
    )
  },
  ul({ children, ...props }) {
    return (
      <ul className="list-disc list-inside my-1.5 space-y-0.5 text-sm text-gray-200" {...props}>
        {children}
      </ul>
    )
  },
  ol({ children, ...props }) {
    return (
      <ol className="list-decimal list-inside my-1.5 space-y-0.5 text-sm text-gray-200" {...props}>
        {children}
      </ol>
    )
  },
  h1({ children, ...props }) {
    return (
      <h1 className="text-base font-semibold text-gray-100 my-3 first:mt-0" {...props}>
        {children}
      </h1>
    )
  },
  h2({ children, ...props }) {
    return (
      <h2 className="text-sm font-semibold text-gray-100 my-2.5" {...props}>
        {children}
      </h2>
    )
  },
  h3({ children, ...props }) {
    return (
      <h3 className="text-[13px] font-semibold text-gray-100 my-2" {...props}>
        {children}
      </h3>
    )
  },
  blockquote({ children, ...props }) {
    return (
      <blockquote
        className="border-l-2 border-blue-400/30 pl-3 my-2 text-sm text-gray-400 italic"
        {...props}
      >
        {children}
      </blockquote>
    )
  },
  hr(props) {
    return <hr className="my-3 border-white/[0.06]" {...props} />
  },
}

export function Markdown({ children, className = '' }: MarkdownProps) {
  return (
    <div className={`text-sm text-gray-200 leading-relaxed ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={components}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
}