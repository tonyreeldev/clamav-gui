import { Link, useLocation } from "react-router";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { SIDEBAR_FOOTER_LINKS, SIDEBAR_LINKS } from "@/lib/constants/links";
import { SidebarLink } from "@/lib/types/enums";
import { useSettings } from "@/context/settings";
import { useQuarantineCount } from "@/context/antivirus/quarantine-count";
import { useRealtimeScan } from "@/context/antivirus/real-time";
import { cn } from "@/lib/utils";
import Logo from "./logo";

// Top navigation that replaces the old left sidebar: brand, page tabs, protection status, utilities.
export default function TopNav(){
     const {pathname} = useLocation();
     const {settings} = useSettings();
     const {quarantineCount} = useQuarantineCount();
     const {t} = useTranslation();
     const links = useMemo(()=>!settings.enableSchedulerUI ? SIDEBAR_LINKS.filter(val=>val.href!=="/scheduler") : SIDEBAR_LINKS,[settings.enableSchedulerUI]);
     const isActive = (href: string) => href==="/" ? pathname==="/" : pathname.startsWith(href);
     // What the backend actually runs, not just the saved preference (start can fail).
     const {enabled: isProtected} = useRealtimeScan();
     return (
          <header className="sticky top-0 z-40 border-b bg-background/85 backdrop-blur">
               <div className="mx-auto max-w-6xl flex items-center gap-4 px-4 h-14">
                    <Link to="/" className="shrink-0" aria-label="ClamAV GUI">
                         <Logo/>
                    </Link>
                    <nav className="flex-1 min-w-0 overflow-x-auto">
                         <ul className="flex items-center gap-1 w-max mx-auto">
                              {links.map(({Icon, name, href})=>(
                                   <li key={href}>
                                        <Link
                                             to={href}
                                             title={t(`sidebar.${name}`)}
                                             aria-current={isActive(href) ? "page" : undefined}
                                             className={cn(
                                                  "relative flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm transition-colors",
                                                  isActive(href) ? "bg-primary/12 text-primary font-medium" : "text-muted-foreground hover:text-foreground hover:bg-accent"
                                             )}
                                        >
                                             <Icon className="size-4"/>
                                             {/* All six labels only fit from ~1280 px; narrower windows (laptops) show icons only, so no tab is pushed out of view. */}
                                             <span className="sr-only xl:not-sr-only">{t(`sidebar.${name}`)}</span>
                                             {name===SidebarLink.Quarantine && quarantineCount>0 && (
                                                  <span className="ml-0.5 rounded-full bg-destructive px-1.5 text-[10px] leading-4 font-semibold text-white">{quarantineCount}</span>
                                             )}
                                        </Link>
                                   </li>
                              ))}
                         </ul>
                    </nav>
                    <div className="flex items-center gap-1 shrink-0">
                         <span className={cn(
                              "hidden sm:flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium",
                              isProtected ? "border-emerald-500/30 text-emerald-600 dark:text-emerald-400" : "border-red-500/30 text-red-600 dark:text-red-400"
                         )}>
                              <span className={cn("size-1.5 rounded-full", isProtected ? "bg-emerald-500" : "bg-red-500")}/>
                              {t(isProtected ? "status.protected" : "status.unprotected")}
                         </span>
                         {SIDEBAR_FOOTER_LINKS.map(({Icon, name, href})=>(
                              <Link
                                   key={href}
                                   to={href}
                                   title={t(`sidebar.${name}`)}
                                   aria-label={t(`sidebar.${name}`)}
                                   aria-current={isActive(href) ? "page" : undefined}
                                   className={cn(
                                        "grid place-items-center size-9 rounded-full transition-colors",
                                        isActive(href) ? "bg-primary/12 text-primary" : "text-muted-foreground hover:text-foreground hover:bg-accent"
                                   )}
                              >
                                   <Icon className="size-4.5"/>
                              </Link>
                         ))}
                    </div>
               </div>
          </header>
     )
}
