import { afterEach, beforeEach, expect, it, vi } from "vitest";

import { __resetServerEnv } from "@/lib/config/server-env.schema";

let exit: ReturnType<typeof vi.spyOn>;

/** `register` as at a fresh start: the check runs once per module load. */
async function register(): Promise<void> {
  vi.resetModules();
  const { register: run } = await import("../instrumentation");
  await run();
}

beforeEach(() => {
  __resetServerEnv();
  vi.stubEnv("NEXT_RUNTIME", "nodejs");
  vi.stubEnv("YOG_API_URL", "http://api.test");
  exit = vi.spyOn(process, "exit").mockImplementation(() => undefined as never);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  __resetServerEnv();
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
});

// Mutations: `register` reads nothing, or throws instead of exiting — Next stays up and answers 500.
it("stops the process at start-up without a key, saying why", async () => {
  vi.stubEnv("YOG_API_KEY", "");
  await register();
  expect(exit).toHaveBeenCalledWith(1);
  expect(console.error).toHaveBeenCalledWith(expect.stringMatching(/YOG_API_KEY/));
});

it("lets a well-formed configuration start", async () => {
  vi.stubEnv("YOG_API_KEY", `yog_0badc0de_${"a".repeat(64)}`);
  await register();
  expect(exit).not.toHaveBeenCalled();
});
