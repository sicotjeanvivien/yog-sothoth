import * as z from "zod";
import { BigDecimal, Rfc3339 } from "./shared";

export const PriceSchema = z.object({
  usd: BigDecimal,
  source: z.string(),
  fetchedAt: Rfc3339
});

export type PriceResponse = z.infer<typeof PriceSchema>;
