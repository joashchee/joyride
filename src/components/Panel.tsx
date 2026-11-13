import type { ReactNode } from "react";

/** A text-mode window: a double-line frame with its title set into the top
 *  edge, a close box ([■]) at the top left, and optional buttons at the top
 *  right. Escape closes it. The frame is drawn by CSS in the panel's ring of
 *  cells (src/ansiapps-theme.css). */
export default function Panel(props: {
  title: string;
  /** Extra class, e.g. "panel-docs", which sets the panel's size in rows. */
  kind: string;
  onClose?: () => void;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const { title, kind, onClose, actions, children } = props;
  return (
    <section className={"panel " + kind} aria-label={title} onKeyDown={(e) => e.key === "Escape" && onClose?.()}>
      {onClose && (
        <button className="panel-close" onClick={onClose} aria-label={`Close ${title}`} title={`Close ${title}`}>
          [■]
        </button>
      )}
      <h2 className="panel-title">{title}</h2>
      {actions && <div className="panel-actions">{actions}</div>}
      <div className="panel-body">{children}</div>
    </section>
  );
}
