// The clear resize band rendered between the two halves of a `useVerticalSplit` stack: a full-width
// divider with a centered grip, drag up/down to resize. Pure view — wire it to a `VerticalSplit`'s
// `onHandleDown`. Shared by the Data Studio panel builder and the Rules editor so both feel the same.

interface Props {
  onPointerDown: (e: React.PointerEvent) => void;
  label?: string;
}

export function SplitHandle({ onPointerDown, label = "resize" }: Props) {
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label={label}
      onPointerDown={onPointerDown}
      className="group relative my-1 flex h-3 shrink-0 cursor-row-resize items-center justify-center border-y border-border bg-panel transition-colors hover:bg-accent/20"
    >
      <span className="h-0.5 w-8 rounded-full bg-border transition-colors group-hover:bg-primary" />
    </div>
  );
}
