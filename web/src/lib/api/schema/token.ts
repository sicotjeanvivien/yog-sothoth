import * as z from "zod";
import { PriceSchema } from "./price";

export const TokenSchema = z.object({
  // null until the pool's mints are resolved by yog-context.
  mint: z.string().nullable(),
  symbol: z.string().nullable(),
  name: z.string().nullable(),
  decimals: z.number(),
  logoUri: z.url().nullable(),
  price: PriceSchema.nullable(),
});

export type TokenResponse = z.infer<typeof TokenSchema>;
