import { useEffect, useState } from "react";
import Splash from "./components/Splash";
import { coreInfo, type CoreInfo } from "./lib/backend";

// What the first screen needs, in order. Heavy things (Monaco, the language
// server, toolchain checks) load after the editor is usable, not here.
const STEPS = ["Opening the engine", "Ready"] as const;

export default function App() {
  const [step, setStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<CoreInfo | null>(null);
  const [leaving, setLeaving] = useState(false);
  const [gone, setGone] = useState(false);

  useEffect(() => {
    let cancelled = false;
    coreInfo()
      .then((i) => {
        if (cancelled) return;
        setInfo(i);
        setStep(1);
        setLeaving(true);
        setTimeout(() => !cancelled && setGone(true), 220);
      })
      .catch((e) => !cancelled && setError(`JoyrIDE could not start.\n${String(e)}`));
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <>
      {!gone && (
        <Splash
          label={STEPS[step]}
          progress={step / (STEPS.length - 1)}
          error={error}
          leaving={leaving}
        />
      )}
      <main className="shell" aria-label="JoyrIDE">
        <h1>JoyrIDE</h1>
        <p>Core {info?.version ?? "…"}. The editor arrives next.</p>
      </main>
    </>
  );
}
