/**
 * Small real logos for the tools Ember watches, at text size.
 *
 * The brand marks are the material here; a monogram chip is the fallback for
 * anything without one.
 */

import type { AgentId } from "../types";

import antigravitySvg from "../assets/agents/antigravity-color.svg";
import claudeCodeSvg from "../assets/agents/claudecode-color.svg";
import clineSvg from "../assets/agents/cline.svg";
import codexSvg from "../assets/agents/codex-color.svg";
import ohMyPiSvg from "../assets/agents/oh-my-pi.svg";
import opencodeSvg from "../assets/agents/opencode.svg";
import piSvg from "../assets/agents/pi.svg";

const ICONS: Record<string, { src: string; alt: string; mono?: boolean }> = {
  "claude-code": { src: claudeCodeSvg, alt: "Claude Code" },
  claudecode: { src: claudeCodeSvg, alt: "Claude Code" },
  codex: { src: codexSvg, alt: "Codex" },
  opencode: { src: opencodeSvg, alt: "OpenCode", mono: true },
  pi: { src: piSvg, alt: "pi" },
  "oh-my-pi": { src: ohMyPiSvg, alt: "Oh My Pi" },
  antigravity: { src: antigravitySvg, alt: "Antigravity" },
  cline: { src: clineSvg, alt: "Cline", mono: true },
};

export function AgentIcon({
  agent,
  size = 16,
  className = "",
  monogram,
}: {
  agent: AgentId | string;
  size?: number;
  className?: string;
  monogram?: string;
}) {
  const norm = String(agent).toLowerCase().replace(/_/g, "-");
  const icon = ICONS[norm];

  if (!icon) {
    const text = (monogram ?? String(agent)).replace(/[^a-z0-9]/gi, "").slice(0, 2).toUpperCase();
    return (
      <span className={`monogram-chip ${className}`} aria-hidden="true">
        {text || "?"}
      </span>
    );
  }

  return (
    <img
      className={`agent-icon${icon.mono ? " agent-icon--mono" : ""}${
        className ? ` ${className}` : ""
      }`}
      src={icon.src}
      alt=""
      width={size}
      height={size}
      loading="eager"
      draggable={false}
      style={{ width: size, height: size, objectFit: "contain", display: "block" }}
    />
  );
}
