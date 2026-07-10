// MarkdownBody — render a markdown string to styled React nodes (reports scope). Ported from
// lazybones' `markdown.tsx`: `react-markdown` + `remark-gfm` (tables, task lists, strikethrough),
// each element styled inline (no `@tailwindcss/typography` in this repo). XSS-safe by construction:
// react-markdown escapes raw HTML (no `rehype-raw`). Colours INHERIT from the container so the same
// body reads correctly on the dark editor panel AND the white A4 report sheet. One responsibility:
// markdown in, React out — no state, no effects.

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function MarkdownBody({ children, label }: { children: string; label?: string }) {
  return (
    <div aria-label={label} data-testid="markdown-body" className="text-sm leading-relaxed">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          h1: (p) => <h1 className="mb-2 mt-4 text-xl font-semibold tracking-tight first:mt-0" {...strip(p)} />,
          h2: (p) => <h2 className="mb-2 mt-4 text-lg font-semibold tracking-tight first:mt-0" {...strip(p)} />,
          h3: (p) => <h3 className="mb-1.5 mt-3 text-sm font-semibold uppercase tracking-wide opacity-80 first:mt-0" {...strip(p)} />,
          p: (p) => <p className="my-2 first:mt-0 last:mb-0" {...strip(p)} />,
          ul: (p) => <ul className="my-2 ml-5 list-disc space-y-1 marker:opacity-60" {...strip(p)} />,
          ol: (p) => <ol className="my-2 ml-5 list-decimal space-y-1 marker:opacity-60" {...strip(p)} />,
          li: (p) => <li className="pl-1" {...strip(p)} />,
          a: (p) => <a className="underline underline-offset-2 opacity-90 hover:opacity-100" target="_blank" rel="noreferrer" {...strip(p)} />,
          blockquote: (p) => <blockquote className="my-2 border-l-2 border-current/30 pl-3 italic opacity-80" {...strip(p)} />,
          hr: () => <hr className="my-4 border-current/20" />,
          table: (p) => <table className="my-2 w-full border-collapse text-xs" {...strip(p)} />,
          th: (p) => <th className="border border-current/20 px-2 py-1 text-left font-semibold" {...strip(p)} />,
          td: (p) => <td className="border border-current/20 px-2 py-1" {...strip(p)} />,
          code: ({ className: c, children: kids, ...p }) => {
            const inline = !String(c ?? "").includes("language-");
            return inline ? (
              <code className="rounded bg-black/10 px-1 py-0.5 font-mono text-[0.85em]" {...strip(p)}>
                {kids}
              </code>
            ) : (
              <code className={`${c ?? ""} block overflow-x-auto rounded bg-black/10 p-2 font-mono text-xs`} {...strip(p)}>
                {kids}
              </code>
            );
          },
          pre: (p) => <pre className="my-2" {...strip(p)} />,
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}

/** Drop react-markdown's non-DOM `node` prop before spreading onto an element. */
function strip<T extends { node?: unknown }>({ node: _node, ...rest }: T) {
  return rest;
}
