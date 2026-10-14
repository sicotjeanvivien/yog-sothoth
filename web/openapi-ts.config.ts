import { defineConfig } from "@hey-api/openapi-ts";

// The API contract, generated: `npm run gen:api` rewrites `src/lib/api/schema/`
// whole from `crates/api/openapi.json`. Nothing in that folder is edited by hand.
export default defineConfig({
  input: "../crates/api/openapi.json",
  output: "src/lib/api/schema",
  plugins: [
    {
      name: "zod",
      comments: false,
      dates: { offset: true },
      definitions: { types: { infer: { case: "PascalCase", name: "{{name}}" } } },
    },
  ],
});
