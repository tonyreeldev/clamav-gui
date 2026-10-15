// Minimal design: every accent shares one neutral, slightly cool base;
// only the primary color, focus ring and chart ramp change per accent.
interface Accent {
     hue: number,
     chroma: number,
     light: string,
     dark: string,
}

const accent = (hue: number, chroma: number, lightL: number, darkL: number): Accent => ({
     hue,
     chroma,
     light: `oklch(${lightL} ${chroma} ${hue})`,
     dark: `oklch(${darkL} ${chroma} ${hue})`,
});

function palette({hue, chroma, light, dark}: Accent){
     const light_ = {
          background: "oklch(0.985 0.002 220)",
          foreground: "oklch(0.21 0.012 235)",
          card: "oklch(1 0 0)",
          cardForeground: "oklch(0.21 0.012 235)",
          popover: "oklch(1 0 0)",
          popoverForeground: "oklch(0.21 0.012 235)",
          primary: light,
          primaryForeground: "oklch(0.985 0.005 220)",
          secondary: "oklch(0.96 0.004 220)",
          secondaryForeground: "oklch(0.25 0.012 235)",
          muted: "oklch(0.96 0.004 220)",
          mutedForeground: "oklch(0.52 0.014 235)",
          accent: "oklch(0.955 0.006 220)",
          accentForeground: "oklch(0.21 0.012 235)",
          destructive: "oklch(0.58 0.22 27)",
          border: "oklch(0.925 0.004 220)",
          input: "oklch(0.91 0.004 220)",
          ring: light,
     };
     const dark_ = {
          background: "oklch(0.165 0.008 235)",
          foreground: "oklch(0.96 0.003 220)",
          card: "oklch(0.2 0.009 235)",
          cardForeground: "oklch(0.96 0.003 220)",
          popover: "oklch(0.2 0.009 235)",
          popoverForeground: "oklch(0.96 0.003 220)",
          primary: dark,
          primaryForeground: "oklch(0.18 0.02 235)",
          secondary: "oklch(0.25 0.01 235)",
          secondaryForeground: "oklch(0.96 0.003 220)",
          muted: "oklch(0.25 0.01 235)",
          mutedForeground: "oklch(0.7 0.012 230)",
          accent: "oklch(0.24 0.01 235)",
          accentForeground: "oklch(0.96 0.003 220)",
          destructive: "oklch(0.68 0.19 22)",
          border: "oklch(1 0 0 / 8%)",
          input: "oklch(1 0 0 / 12%)",
          ring: dark,
     };
     return {
          light: {
               ...light_,
               sidebar: {
                    background: light_.background,
                    foreground: light_.foreground,
                    primary: light,
                    primaryForeground: light_.primaryForeground,
                    accent: light_.accent,
                    accentForeground: light_.accentForeground,
                    border: light_.border,
                    ring: light,
               }
          },
          dark: {
               ...dark_,
               sidebar: {
                    background: dark_.background,
                    foreground: dark_.foreground,
                    primary: dark,
                    primaryForeground: dark_.primaryForeground,
                    accent: dark_.accent,
                    accentForeground: dark_.accentForeground,
                    border: dark_.border,
                    ring: dark,
               }
          },
          charts: [0.85, 0.74, 0.64, 0.54, 0.44].map(l => `oklch(${l} ${chroma} ${hue})`),
     };
}

export const COLORS = {
     teal: palette(accent(200, 0.11, 0.58, 0.76)),
     blue: palette(accent(262, 0.19, 0.55, 0.7)),
     green: palette(accent(152, 0.15, 0.58, 0.74)),
     rose: palette(accent(12, 0.19, 0.6, 0.72)),
}
