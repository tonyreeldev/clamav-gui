import * as z from "zod";
import { SCAN_SETTINGS } from "../constants/settings/scan-options";

const yesNo = z.boolean().optional();
const choice012 = z.union([z.literal(0), z.literal(1), z.literal(2)]).optional();
const pathInput = z.string().trim().optional();

const ScanProfileShape = {
     // File System
     recursive: yesNo,
     crossFs: yesNo,
     followDirSymlinks: choice012,
     followFileSymlinks: choice012,

     // Output
     quiet: yesNo,
     verbose: yesNo,
     infected: yesNo,
     noSummary: yesNo,
     bell: yesNo,
     debug: yesNo,

     // Detection
     algorithmicDetection: yesNo,
     heuristicAlerts: yesNo,
     heuristicPrecedence: yesNo,
     detectPUA: yesNo,
     detectStructured: yesNo,
     structuredSSNFormat: choice012,
     structuredSSNCount: z.number().min(SCAN_SETTINGS.structuredSSNCount.value.min).max(SCAN_SETTINGS.structuredSSNCount.value.max).optional(),
     structuredCCCount: z.number().min(SCAN_SETTINGS.structuredCCCount.value.min).max(SCAN_SETTINGS.structuredCCCount.value.max).optional(),

     // File Types
     scanPE: yesNo,
     scanELF: yesNo,
     scanOLE2: yesNo,
     scanPDF: yesNo,
     scanHTML: yesNo,
     scanArchive: yesNo,
     scanMail: yesNo,
     detectBroken: yesNo,

     // Limits / Performance
     maxFileSize: z.number().min(SCAN_SETTINGS.maxFileSize.value.min).max(SCAN_SETTINGS.maxFileSize.value.max).optional(),
     maxScanSize: z.number().min(SCAN_SETTINGS.maxScanSize.value.min).max(SCAN_SETTINGS.maxScanSize.value.max).optional(),
     alertEncrypted: yesNo,
     maxFiles: z.number().min(SCAN_SETTINGS.maxFiles.value.min).max(SCAN_SETTINGS.maxFiles.value.max).optional(),
     maxRecursion: z.number().min(SCAN_SETTINGS.maxRecursion.value.min).max(SCAN_SETTINGS.maxRecursion.value.max).optional(),
     maxDirRecursion: z.number().min(SCAN_SETTINGS.maxDirRecursion.value.min).max(SCAN_SETTINGS.maxDirRecursion.value.max).optional(),

     // Advanced
     noCerts: yesNo,
     disableCache: yesNo,
     officialDbOnly: yesNo,
     leaveTemps: yesNo,
     bytecode: yesNo,
     bytecodeUnsigned: yesNo,
     bytecodeTimeout: z.number().min(SCAN_SETTINGS.bytecodeTimeout.value.min).max(SCAN_SETTINGS.bytecodeTimeout.value.max).optional(),
     databasePath: pathInput,
     tempDir: pathInput,
} satisfies {
     [K in keyof typeof SCAN_SETTINGS]: z.ZodType<unknown>;
};

export const ScanProfileSchema = z.object(ScanProfileShape);