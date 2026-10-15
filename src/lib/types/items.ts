import { QuickAccessLink, SidebarLink, ScanType } from "./enums";
import { LucideIcon } from "lucide-react";

export interface IScanMenuItem{
     type: ScanType,
     Icon: LucideIcon
}
export interface IQuickAccessItem{
     type: QuickAccessLink
     href: string,
     Icon: LucideIcon,
     openDialogType: "none" | "file" | "folder"
}
export interface ISidebarItem{
     name: SidebarLink,
     href: string,
     Icon: LucideIcon,
}
export interface ISpecialThanksItem{
     handle: string,
     link: string,
     note: "early-test" | "bug-report-test"
}