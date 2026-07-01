// The cron-builder authoring component — a thin wrapper around `react-js-cron` (reminders scope's
// pinned React cron builder). Most users do not read cron, so the UI authoring surface is a visual
// builder that reads/writes a standard 5-field cron string (lossless round-trip). It is antd-based,
// so the wrapper scopes antd's ConfigProvider to THIS subtree (the shell's global theme is Tailwind
// + shadcn — antd is NOT pulled into the global theme, per the scope decision). One component, one
// concern (FILE-LAYOUT): a labeled cron field that round-trips a string.

import { ConfigProvider, theme as antdTheme } from "antd";
import { Cron } from "react-js-cron";
import "react-js-cron/dist/styles.css";

interface Props {
  /** The current 5-field cron string (e.g. `0 8 * * 0,1`). */
  value: string;
  /** Called with the new cron string on every edit (lossless round-trip). */
  onChange: (value: string) => void;
}

/** A labeled visual cron authoring field. Renders the antd `Cron` builder scoped under a local
 *  ConfigProvider so antd never touches the global theme. */
export function CronBuilder({ value, onChange }: Props) {
  return (
    <ConfigProvider
      theme={{
        algorithm: antdTheme.darkAlgorithm,
        token: { colorPrimary: "#f59e0b", borderRadius: 6 },
      }}
    >
      <div className="cron-builder">
        <Cron value={value} setValue={onChange} clockFormat="12-hour-clock" />
      </div>
    </ConfigProvider>
  );
}
