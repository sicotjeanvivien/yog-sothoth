import * as z from "zod";
import { PriceSchema } from "./price";

export const TokenSchema = z.object({
  mint: z.string(),
  symbol: z.string().nullable(),
  name: z.string().nullable(),
  decimals: z.number(),
  logoUri: z.url().nullable(),
  price: PriceSchema.nullable(),
});

export type TokenResponse = z.infer<typeof TokenSchema>;
