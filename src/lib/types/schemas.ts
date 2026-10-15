import * as z from "zod"
import { getClearByDateSchema, getClearByRangeSchema, getPathFormSchema, getSchedulerSchema } from "../schemas";
import { ScanProfileSchema } from "../schemas/scan-profile";

export type SchedulerType = z.infer<
     Awaited<ReturnType<typeof getSchedulerSchema>>
>
export type PathFormType = z.infer<
     Awaited<ReturnType<typeof getPathFormSchema>>
>
export type ClearByDateType = z.infer<
     Awaited<ReturnType<typeof getClearByDateSchema>>
>
export type ClearByRangeType = z.infer<
     Awaited<ReturnType<typeof getClearByRangeSchema>>
>
export type ScanProfileType = z.infer<typeof ScanProfileSchema>