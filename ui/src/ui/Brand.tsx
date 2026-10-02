// Brand architecture (roadmap 2.1b): three levels, each in its own place.
//  1. Clinic   — ClinicMark/Avatar below, everywhere in the working UI.
//  2. Product  — Artaveo Dental (branding/artaveo-dental/): app icon, sidebar
//     footer mark, splash screen, login screen, About page.
//  3. Company  — Artaveo (branding/artaveo/): only the small "by Artaveo"
//     credit line on the About page.
import dentalIcon from "../assets/brand-dental/icon.png";
import dentalLockupLight from "../assets/brand-dental/lockup-light.png";
import dentalLockupDark from "../assets/brand-dental/lockup-dark.png";
import dentalMonoWhite from "../assets/brand-dental/mono-white.png";
import dentalSplashDark from "../assets/brand-dental/splash-dark.png";
import dentalPrimaryLight from "../assets/brand-dental/primary-stacked-light.png";
import companyIconSvg from "../assets/brand/icon.svg";
import companyMonoWhite from "../assets/brand/mono-white.svg";

export const brandAssets = {
  icon: dentalIcon,
  lockupLight: dentalLockupLight,
  lockupDark: dentalLockupDark,
  splashLight: dentalPrimaryLight,
  splashDark: dentalSplashDark,
};

/** Artaveo Dental horizontal lockup (mark + wordmark), the right file for the current theme. */
export function DentalLockup({ height = 30 }: { height?: number }) {
  return (
    <>
      <img className="lockup only-light" src={dentalLockupLight} alt="Artaveo Dental" style={{ height }} />
      <img className="lockup only-dark" src={dentalLockupDark} alt="Artaveo Dental" style={{ height }} />
    </>
  );
}

/** Artaveo Dental flat mark — sidebar footer, small product credits. */
export function DentalMark({ size = 18 }: { size?: number }) {
  return (
    <>
      <img className="only-light" src={dentalIcon} alt="" width={size} height={size} />
      <img className="only-dark" src={dentalMonoWhite} alt="" width={size} height={size} />
    </>
  );
}

/** Premium Artaveo Dental mark for the splash screen and About hero (opaque, theme-matched panel). */
export function DentalPremiumArt({ alt = "", className = "" }: { alt?: string; className?: string }) {
  return (
    <>
      <img className={`only-light ${className}`} src={dentalPrimaryLight} alt={alt} />
      <img className={`only-dark ${className}`} src={dentalSplashDark} alt={alt} />
    </>
  );
}

/** The Artaveo company mark — "by Artaveo" credit on the About page only. */
export function ArtaveoMark({ size = 18 }: { size?: number }) {
  return (
    <>
      <img className="only-light" src={companyIconSvg} alt="" width={size} height={size} />
      <img className="only-dark" src={companyMonoWhite} alt="" width={size} height={size} />
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
