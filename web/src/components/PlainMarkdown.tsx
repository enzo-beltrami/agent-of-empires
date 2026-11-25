import type { ComponentPropsWithoutRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkBreaks from "remark-breaks";
import remarkGfm from "remark-gfm";

/** Markdown outside the structured view, whose `<Markdown>` needs a provider not mounted here.
 *  Typed text, so single newlines are kept as breaks. */
export function PlainMarkdown({ text }: { text: string }) {
  return (
    <div className="acp-markdown break-words text-[13px] leading-relaxed text-text-primary">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]} components={{ a: NewTabLink }}>
        {text}
      </ReactMarkdown>
    </div>
  );
}

function NewTabLink({ node: _node, ...props }: ComponentPropsWithoutRef<"a"> & { node?: unknown }) {
  return <a {...props} target="_blank" rel="noopener noreferrer" />;
}
