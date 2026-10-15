import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import React from "react";
import { cn } from "@/lib/utils";
import { DesignType } from "@/lib/types";
import { LucideIcon } from "lucide-react";

interface Props{
     Icon: LucideIcon
     title: string,
     children: React.ReactNode,
     className?: string,
     description?: string,
     button?: React.JSX.Element,
     type?: DesignType
}
export default function SettingsItem({children, Icon, title, className, description, button, type="default"}: Props){
     const borderColor = cn(type==="default" ? "border-border" : "border-destructive")
     return (
          <Card className={borderColor}>
               <CardHeader>
                    <div className="flex items-between justify-center gap-2 w-full">
                         <div className="space-y-2 w-full">
                              <CardTitle className="flex items-center gap-2.5 font-medium"><span className="grid place-items-center size-8 rounded-lg bg-primary/10 text-primary"><Icon className="size-4"/></span> {title}</CardTitle>
                              {!!description && (
                                   <CardDescription>{description}</CardDescription>
                              )}
                         </div>
                         {button}
                    </div>
               </CardHeader>
               <CardContent className={className}>
                    {children}
               </CardContent>
          </Card>
     )
}