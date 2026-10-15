import { ScanType } from "@/lib/types/enums";
import { cn } from "@/lib/utils";
import { useTranslation } from "react-i18next";
import { IScanMenuItem } from "@/lib/types/items";

interface ScanMenuCardProps{
     data: IScanMenuItem,
     currScanType: ScanType,
     onScanTypeChange: (scanType: ScanType) => void
}
export default function ScanMenuCard({data, currScanType, onScanTypeChange}: ScanMenuCardProps){
     const {t} = useTranslation("scan");
     return data.type!==ScanType.None ? (
          <button type="button" aria-pressed={currScanType===data.type} className={cn("p-4 border bg-card text-card-foreground rounded-xl w-full flex items-center gap-4 text-left transition-colors cursor-pointer focus-visible:outline-2 focus-visible:outline-ring",currScanType!==data.type ? "border-border hover:border-primary/50 hover:bg-accent/40" : "border-primary bg-primary/5")} onClick={()=>onScanTypeChange(data.type)}>
               <span className="grid place-items-center size-11 shrink-0 rounded-lg bg-primary/10 text-primary">
                    <data.Icon className="size-5"/>
               </span>
               <span className="space-y-0.5">
                    <span className="block font-medium md:text-lg">{t(`scan-type.${data.type}.name`)}</span>
                    <span className="block text-sm text-muted-foreground">{t(`scan-type.${data.type}.desc`)}</span>
               </span>
          </button>
     ) : null
}