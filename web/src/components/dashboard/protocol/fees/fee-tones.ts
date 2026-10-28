/** The tints of a fee bar, read by the server's legend and table and the client's bars alike. */

import type { FeePart } from "@/lib/protocol/fees";

export const FEE_TONE: Record<FeePart, string> = {
  lp: "bg-dash-fee-lp",
  protocol: "bg-dash-fee-protocol",
  referral: "bg-dash-fee-referral",
};
