/**
 * Core interactive primitives: Button, Switch, sliding Segmented control, Notice.
 */

import {
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

type ButtonVariant = "primary" | "quiet" | "ghost";

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
      <span className="control-label">{children}</span>
    </button>
  );
}

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

/**
 * Segmented control with a smooth, pixel-measured sliding thumb.
 * Snaps on first layout so it never flies in from x=0, then glides smoothly
 * across selections using GPU transforms.
 */
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
  const groupRef = useRef<HTMLDivElement | null>(null);
  const buttonRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const hasMeasured = useRef(false);
  const [thumb, setThumb] = useState<{ left: number; width: number; ready: boolean; animate: boolean }>({
    left: 0,
    width: 0,
    ready: false,
    animate: false,
  });

  const activeIndex = Math.max(
    0,
    options.findIndex((o) => o.value === value),
  );

  const measure = useCallback(
    (animate: boolean) => {
      const group = groupRef.current;
      const btn = buttonRefs.current[activeIndex];
      if (!group || !btn) return;
      const groupRect = group.getBoundingClientRect();
      const btnRect = btn.getBoundingClientRect();
      const left = btnRect.left - groupRect.left - group.clientLeft;
      const width = btnRect.width;
      if (width <= 0) return;
      setThumb((prev) => {
        if (
          prev.ready &&
          Math.abs(prev.left - left) < 0.05 &&
          Math.abs(prev.width - width) < 0.05
        ) {
          return prev;
        }
        return { left, width, ready: true, animate: prev.ready && animate };
      });
    },
    [activeIndex],
  );

  useLayoutEffect(() => {
    measure(hasMeasured.current);
    hasMeasured.current = true;
  }, [measure, options.length]);

  useEffect(() => {
    const group = groupRef.current;
    if (!group) return;
    const ro = new ResizeObserver(() => measure(false));
    ro.observe(group);
    void document.fonts?.ready?.then(() => measure(false));
    return () => ro.disconnect();
  }, [measure]);

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    let next = -1;
    if (event.key === "ArrowRight") next = (index + 1) % options.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + options.length) % options.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.length - 1;
    if (next >= 0) {
      event.preventDefault();
      onChange(options[next].value);
      buttonRefs.current[next]?.focus();
    }
  };

  return (
    <div className="seg" role="radiogroup" aria-label={label} ref={groupRef}>
      <span
        aria-hidden="true"
        className={`seg__thumb${thumb.ready ? " is-ready" : ""}${thumb.animate ? " is-animated" : ""}`}
        style={{
          width: `${thumb.width}px`,
          transform: `translate3d(${thumb.left}px, 0, 0)`,
        }}
      />
      {options.map((o, index) => {
        const active = value === o.value;
        return (
          <button
            key={o.value}
            ref={(el) => {
              buttonRefs.current[index] = el;
            }}
            type="button"
            role="radio"
            aria-checked={active}
            tabIndex={active ? 0 : -1}
            className={`seg__btn${active ? " is-active" : ""}`}
            onClick={() => onChange(o.value)}
            onKeyDown={(e) => onKeyDown(e, index)}
          >
            <span className="control-label">{o.label}</span>
          </button>
        );
      })}
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
