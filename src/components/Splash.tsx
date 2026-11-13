/** The startup screen: the same markup as the static splash in index.html,
 *  plus determinate progress and any load error (never a blank window). */
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
      <img className="splash-mark" src="/favicon.svg" alt="" />
      <div className="splash-name">
        JoyrIDE<span className="splash-accent">.</span>
      </div>
      {error ? (
        <div className="splash-error" role="alert">{error}</div>
      ) : (
        <>
          <progress
            className="splash-progress"
            aria-label="Starting JoyrIDE"
            {...(progress === null ? {} : { value: progress, max: 1 })}
          />
          <div className="splash-label">{label}</div>
        </>
      )}
    </div>
  );
}
