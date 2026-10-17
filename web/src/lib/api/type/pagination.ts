import type * as z from "zod";

import type { zListSignalsQuery } from "../schema/zod.gen";

// The values come from the contract: a value renamed server-side breaks the build here.
type ListSignalsQuery = z.infer<typeof zListSignalsQuery>;

/** `next` moves further into the list, `prev` back; `next` when unspecified. */
export type PageDir = NonNullable<ListSignalsQuery["dir"]>;

export type PagePosition = NonNullable<ListSignalsQuery["position"]>;
