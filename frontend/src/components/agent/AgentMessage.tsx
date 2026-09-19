import { memo } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import styles from "./AgentPanel.module.css";

export const AgentMessage = memo(function AgentMessage({
  text,
  streaming,
}: {
  text: string;
  streaming?: boolean;
}) {
  return (
    <div className={styles.markdown} aria-busy={streaming || undefined}>
      <Markdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          // Generated links never navigate the DAW or fetch remote tracking images.
          a: ({ children, href }) => <span title={href}>{children}</span>,
          img: ({ alt }) => <span>{alt}</span>,
        }}
      >
        {text}
      </Markdown>
      {streaming && (
        <span
          className={styles.streamCursor}
          data-motion="caret"
          aria-label="Writing response"
        />
      )}
    </div>
  );
});

export function humanStatus(
  status: string,
  running: boolean,
  error: boolean,
): string {
  if (error) return "Could not finish this request";
  if (/^Stopping/.test(status)) return "Stopping…";
  if (/^Stopped/.test(status)) return "Stopped";
  if (!running) return "Ready when you are";
  if (/^(Starting|Checking|.*connecting)/i.test(status)) return "Connecting…";
  if (/strip\.setPlugin|strip\.setInstrument|plugin\./.test(status))
    return "Loading an instrument or effect…";
  if (/strip\.|master\.|preset\./.test(status)) return "Adjusting the sound…";
  if (/clip\.|note\.|track\.add/.test(status)) return "Working on your music…";
  if (/transport\./.test(status)) return "Preparing playback…";
  if (/session\.(save|export|bounce)/.test(status))
    return "Preparing your audio files…";
  if (/session\.|track\.|audio\./.test(status)) return "Checking your project…";
  if (/^Finished/.test(status)) return "Tool finished · thinking…";
  return "Thinking…";
}
