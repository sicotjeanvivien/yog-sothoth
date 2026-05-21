/**
 * High-level fetcher for `GET /api/pools/{address}/latest-state`.
 *
 * Returns the projected current state of the pool (latest reserves,
 * last sqrt_price observed from a swap, last liquidity observed from
 * a liquidity event). Throws `ApiClientError` with `status: 404` when
 * no swap or liquidity event has been observed for this pool yet.
 *
 * The 404 condition does NOT imply the pool is unknown: a pool may
 * exist in `GET /api/pools/{address}` (because a ClaimPositionFee or
 * ClaimReward event touched it) without having a row in the
 * pool_current_state projection. See the CQRS design note in
 * `crates/core/src/domain/pool_current_state.rs`.
 */

import { apiGet } from "./client";
import { isValidPoolAddress } from "./pool";
import {
  PoolCurrentStateSchema,
  type PoolCurrentStateResponse,
} from "./schema/pool-current-state";

/**
 * Fetch the latest projected state of a pool.
 *
 * @throws TypeError if `address` is syntactically invalid.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 *         A 404 from yog-api surfaces as `kind: "http", status: 404`
 *         and means "no swap or liquidity event observed yet" — see
 *         the file-level note.
 */
export async function fetchPoolLatestState(
  address: string,
): Promise<PoolCurrentStateResponse> {
  if (!isValidPoolAddress(address)) {
    throw new TypeError(`invalid pool address: ${address}`);
  }

  return apiGet(
    `/api/pools/${address}/latest-state`,
    {},
    PoolCurrentStateSchema,
  );
}