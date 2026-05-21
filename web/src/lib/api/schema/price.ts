import * as z from "zod";
import { Rfc3339 } from "./shared";

export const PriceSchema = z.object({
  usd: z.string(),
  source: z.string(),
  fetchedAt: Rfc3339
});

export type PriceResponse = z.infer<typeof PriceSchema>;