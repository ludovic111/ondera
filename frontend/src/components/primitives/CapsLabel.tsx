import type { CSSProperties, ReactNode } from "react";

/** Small-caps section label from the type scale. */
export function CapsLabel({
  children,
  style,
}: {
  children: ReactNode;
  style?: CSSProperties | undefined;
}) {
  return (
    <div className="t-caps" style={style}>
      {children}
    </div>
  );
}
