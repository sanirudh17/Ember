import awake from "../assets/interface/awake.svg";
import agents from "../assets/interface/agents.svg";
import settings from "../assets/interface/settings.svg";

const GLYPHS = { awake, agents, settings };

/** MonoCode's MIT-licensed glyphs, tinted through the current theme. */
export function InterfaceIcon({ name, size = 19 }: { name: keyof typeof GLYPHS; size?: number }) {
  return <span className="interface-icon" aria-hidden="true" style={{
    width: size, height: size, maskImage: `url("${GLYPHS[name]}")`,
    WebkitMaskImage: `url("${GLYPHS[name]}")`,
  }} />;
}
