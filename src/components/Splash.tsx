/** The startup screen: the same markup as the static splash in index.html
 *  (keep them in step), plus determinate progress and any load error (never a
 *  blank window). Text mode: a blue screen, a small ANSI joystick, a block
 *  progress bar that jumps to each new width. */
export default function Splash(props: {
  label: string;
  /** 0..1 when the steps are known. */
  progress: number | null;
  error: string | null;
  leaving: boolean;
}) {
  const { label, progress, error, leaving } = props;
  return (
    <div className={"splash" + (leaving ? " leaving" : "")} role="status" aria-live="polite">
      <div className="splash-box">
        <pre className="splash-art" aria-hidden="true">
          <span className="r">{"    \u2584\u2588\u2588\u2584    \n    \u2580\u2588\u2588\u2580    \n"}</span>
          {"     \u2588\u2588     \n \u2584\u2584\u2584\u2584\u2588\u2588\u2584\u2584\u2584\u2584 \n"}
          <span className="c">{" \u2588\u2588\u2588"}</span>
          <span className="y">{"\u2588"}</span>
          <span className="c">{"\u2588\u2588"}</span>
          <span className="y">{"\u2588"}</span>
          <span className="c">{"\u2588\u2588\u2588 "}</span>
          {"\n \u2580\u2580\u2580\u2580\u2580\u2580\u2580\u2580\u2580\u2580 "}
        </pre>
        <div className="splash-name">
          JoyrIDE<span className="splash-accent">.</span>
        </div>
        {error ? (
          <div className="splash-error" role="alert">{error}</div>
        ) : (
          <>
            <div
              className={"progress splash-progress" + (progress === null ? " indeterminate" : "")}
              role="progressbar"
              aria-label="Starting JoyrIDE"
              {...(progress === null ? {} : { "aria-valuemin": 0, "aria-valuemax": 100, "aria-valuenow": Math.round(progress * 100) })}
            >
              <div className="progress-fill" style={progress === null ? undefined : { width: Math.round(progress * 32) * 8 }} />
            </div>
            <div className="splash-label">{label}</div>
          </>
        )}
      </div>
    </div>
  );
}
