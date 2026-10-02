// Artaveo appears only on the icon, splash, login and About (roadmap 2.1b,
// branding/artaveo/README.md); the working UI shows the clinic's identity.
import iconSvg from "../assets/brand/icon.svg";
import lockupLight from "../assets/brand/lockup-compact-light.png";
import lockupDark from "../assets/brand/lockup-compact-dark.png";
import master from "../assets/brand/logo-master.png";
import monoWhite from "../assets/brand/mono-white.svg";

export const brandAssets = { icon: iconSvg, lockupLight, lockupDark, master };

/** Horizontal mark + ARTAVEO, the right file for the current theme. */
export function ArtaveoLockup({ height = 30 }: { height?: number }) {
  return (
    <>
      <img className="lockup only-light" src={lockupLight} alt="Artaveo" style={{ height }} />
      <img className="lockup only-dark" src={lockupDark} alt="Artaveo" style={{ height }} />
    </>
  );
}

/** The flat mark; the official single-colour white version on dark surfaces. */
export function ArtaveoMark({ size = 18 }: { size?: number }) {
  return (
    <>
      <img className="only-light" src={iconSvg} alt="" width={size} height={size} />
      <img className="only-dark" src={monoWhite} alt="" width={size} height={size} />
    </>
  );
}

/** First visible letter of a name (skips spaces/ZWNJ), for avatars and the clinic badge. */
export function initial(name: string | null | undefined): string {
  const m = (name ?? "").trim().match(/\p{L}/u);
  return (m ? m[0] : "A").toUpperCase();
}

/** The clinic's logo, or its first letter on the clinic colour (roadmap 2.1b). */
export function ClinicMark({ name, logo, size = "md" }: { name: string | null | undefined; logo?: string | null; size?: "sm" | "md" | "lg" }) {
  return (
    <span className={`clinic-mark ${size === "md" ? "" : size}`} aria-hidden data-testid="clinic-mark">
      {logo ? <img src={logo} alt="" /> : initial(name)}
    </span>
  );
}

export function Avatar({ name, size }: { name: string; size?: "sm" | "lg" }) {
  return <span className={`avatar ${size ?? ""}`} aria-hidden>{initial(name)}</span>;
}
