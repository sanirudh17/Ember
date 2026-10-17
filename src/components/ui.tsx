/**
 * The primitives every screen draws with: one button, one switch, one
 * segmented choice, one notice. Everything else is composition.
 */

import type { ReactNode } from "react";

type ButtonVariant = "primary" | "quiet" | "ghost";

/** The one button shape: 34px, square, a single fill per variant. */
export function Button({
  variant = "quiet",
  size,
  onClick,
  disabled,
  title,
  children,
}: {
  variant?: ButtonVariant;
  size?: "sm";
  onClick?: () => void;
  disabled?: boolean;
  title?: string;
  children: ReactNode;
}) {
  const cls = ["btn", `btn--${variant}`, size === "sm" ? "btn--sm" : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={cls} onClick={onClick} disabled={disabled} title={title}>
      {children}
    </button>
  );
}

/** A switch keeps the switch convention it has everywhere else. */
export function Switch({
  checked,
  onChange,
  disabled,
  label,
  title,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  label: string;
  title?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={title}
      disabled={disabled}
      className={`switch${checked ? " is-on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="switch__thumb" aria-hidden="true" />
    </button>
  );
}

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

/** Three plain choices; the active one is the only filled thing. */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (next: T) => void;
  label: string;
}) {
  return (
    <div className="seg" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          className={`seg__btn${value === o.value ? " is-active" : ""}`}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Notice({
  title,
  text,
  action,
}: {
  title: string;
  text?: string;
  action?: ReactNode;
}) {
  return (
    <div className="notice" role="alert">
      <div className="notice__title">{title}</div>
      {text && <div className="notice__text">{text}</div>}
      {action && <div className="notice__action">{action}</div>}
    </div>
  );
}
