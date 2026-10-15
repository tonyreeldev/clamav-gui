import { useNavigate } from "react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { normalizePaths } from "@/lib/helpers";
import { FsOption } from "@/lib/types";
import { useTranslation } from "react-i18next";
import { IQuickAccessItem } from "@/lib/types/items";

interface OverviewCardProps{
     data: IQuickAccessItem
}
export default function OverviewCard({data}: OverviewCardProps){
     const navigate = useNavigate();
     const {t} = useTranslation("overview")
     const openCustomScan = async(href: string, type: FsOption) => {
          const currPath = await open({
               title: t(`dialog-title.${type}`),
               multiple: type==="folder",
               directory: type==="folder",
          });
          if(!currPath) return;
          const paths = normalizePaths(currPath);
          const params = new URLSearchParams();
          for(const path of paths)
               params.append("path",path);
          navigate(`${href}?${params.toString()}`)
     }
     return (
          <button type="button" className="w-full p-4 rounded-xl bg-card text-card-foreground border text-left flex flex-col gap-3 transition-colors hover:border-primary/50 hover:bg-accent/40 focus-visible:outline-2 focus-visible:outline-ring cursor-pointer" onClick={()=>data.openDialogType==="none" ? navigate(data.href) : openCustomScan(data.href,data.openDialogType)}>
               <span className="grid place-items-center size-10 rounded-lg bg-primary/10 text-primary">
                    <data.Icon className="size-5"/>
               </span>
               <span className="space-y-1">
                    <span className="block font-medium">{t(`${data.type}.name`)}</span>
                    <span className="block text-sm text-muted-foreground leading-snug">{t(`${data.type}.desc`)}</span>
               </span>
          </button>
     )
}