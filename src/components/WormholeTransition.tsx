export type WindowMotion = "opening" | "idle" | "closing";

export function WormholeTransition({ motion }: { motion: WindowMotion }) {
  return (
    <div className={`window-wormhole motion-${motion}`} aria-hidden="true">
      <span className="window-wormhole-core" />
      <span className="window-wormhole-ring ring-one" />
      <span className="window-wormhole-ring ring-two" />
      <span className="window-wormhole-particles">
        <i /><i /><i /><i /><i />
      </span>
    </div>
  );
}
