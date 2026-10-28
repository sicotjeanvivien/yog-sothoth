/**
 * The API contract read for the developers page: each operation, its parameters and the
 * fields of its 200 response. Server-side only — the client gets this digest, not the file.
 */

export type Schema = {
  $ref?: string;
  type?: string | string[];
  format?: string;
  description?: string;
  enum?: string[];
  minimum?: number;
  maximum?: number;
  items?: Schema;
  properties?: Record<string, Schema>;
  required?: string[];
  oneOf?: Schema[];
  anyOf?: Schema[];
  allOf?: Schema[];
};

type Parameter = {
  name: string;
  in: "path" | "query";
  required?: boolean;
  description?: string;
  schema: Schema;
};

type Operation = {
  operationId: string;
  summary?: string;
  description?: string;
  parameters?: Parameter[];
  responses: Record<
    string,
    { description?: string; content?: Record<string, { schema: Schema }> }
  >;
};

export type OpenApi = {
  openapi: string;
  info: { title: string; version: string };
  paths: Record<string, Record<string, Operation>>;
  components: { schemas: Record<string, Schema> };
};

export const FAMILIES = [
  "service",
  "swaps",
  "liquidity",
  "poolCreations",
  "positionsFees",
  "rewards",
  "tvl",
  "signals",
  "announcements",
  "byProtocol",
  "byPool",
  "byToken",
] as const;

export type Family = (typeof FAMILIES)[number];

/** Keyed on the segment after `/api/`: a scoped route belongs to its scope's family. */
const FAMILY_OF_SEGMENT: Record<string, Family> = {
  ingestion: "service",
  network: "service",
  swaps: "swaps",
  "liquidity-changes": "liquidity",
  "pool-changes": "liquidity",
  "pool-creations": "poolCreations",
  "pool-discoveries": "poolCreations",
  "position-changes": "positionsFees",
  "position-fee-claims": "positionsFees",
  "protocol-fee-claims": "positionsFees",
  "reward-changes": "rewards",
  "reward-claims": "rewards",
  tvl: "tvl",
  signals: "signals",
  announcements: "announcements",
  protocols: "byProtocol",
  pools: "byPool",
  tokens: "byToken",
};

/** ⚠️ No fallback family: a new route must be placed, or the page fails to build. */
export function familyOf(path: string): Family {
  const segment = path.split("/")[2] ?? "";
  const family = FAMILY_OF_SEGMENT[segment];
  if (!family) throw new Error(`no family for ${path}`);
  return family;
}

export type EndpointParam = {
  name: string;
  in: "path" | "query";
  required: boolean;
  description: string | null;
  type: "string" | "integer";
  enum: string[] | null;
  min: number | null;
  max: number | null;
};

export type ResponseField = {
  /** `items[].amountA`: `[]` marks an array's items. */
  path: string;
  type: string;
  required: boolean;
  description: string | null;
};

export type Endpoint = {
  id: string;
  method: string;
  path: string;
  family: Family;
  summary: string | null;
  description: string | null;
  params: EndpointParam[];
  responseDescription: string | null;
  schemaName: string | null;
  contentType: string;
  sse: boolean;
  fields: ResponseField[];
};

const SSE = "text/event-stream";
const MAX_DEPTH = 6;

function refName(ref: string): string {
  return ref.slice(ref.lastIndexOf("/") + 1);
}

function isNull(s: Schema): boolean {
  return s.type === "null";
}

/** The schema behind `null`: `[T, "null"]`, or `oneOf`/`anyOf` with a `null` branch. */
function unwrapNull(s: Schema): { schema: Schema; nullable: boolean } {
  if (Array.isArray(s.type) && s.type.includes("null")) {
    const rest = s.type.filter((t) => t !== "null");
    return { schema: { ...s, type: rest.length === 1 && rest[0] ? rest[0] : rest }, nullable: true };
  }
  const union = s.oneOf ?? s.anyOf;
  if (union?.some(isNull)) {
    const variants = union.filter((b) => !isNull(b));
    const [only] = variants;
    if (only && variants.length === 1) return { schema: only, nullable: true };
    const described = s.description ? { description: s.description } : {};
    return { schema: { oneOf: variants, ...described }, nullable: true };
  }
  return { schema: s, nullable: false };
}

function resolve(s: Schema, spec: OpenApi): Schema {
  return s.$ref ? (spec.components.schemas[refName(s.$ref)] ?? {}) : s;
}

/**
 * An object's properties and required names: `allOf` parts merged, and a union's variants too,
 * a name required only where every variant requires it.
 */
function objectOf(s: Schema, spec: OpenApi): Pick<Schema, "properties" | "required"> | null {
  const r = resolve(s, spec);
  const variants = r.oneOf ?? r.anyOf;
  if (variants?.length && !variants.some(isNull)) {
    const parts = variants.map((v) => objectOf(v, spec));
    if (parts.some((p) => p === null)) return null;
    const objects = parts as Pick<Schema, "properties" | "required">[];
    return {
      properties: Object.assign({}, ...objects.map((p) => p.properties)),
      required: objects
        .map((p) => p.required ?? [])
        .reduce((all, next) => all.filter((name) => next.includes(name))),
    };
  }
  if (r.allOf) {
    const parts = r.allOf.map((p) => objectOf(p, spec)).filter((p) => p !== null);
    return {
      properties: Object.assign({}, ...parts.map((p) => p.properties)),
      required: parts.flatMap((p) => p.required ?? []),
    };
  }
  return r.properties ? { properties: r.properties, required: r.required ?? [] } : null;
}

function baseType(s: Schema, spec: OpenApi): string {
  if (s.$ref) {
    const target = resolve(s, spec);
    if (target.enum) return "enum";
    if ((target.oneOf ?? target.anyOf)?.length) return "one of";
    if (objectOf(target, spec)) return "object";
    return refName(s.$ref);
  }
  if ((s.oneOf ?? s.anyOf)?.length) return "one of";
  if (s.allOf) return "object";
  if (s.enum) return "enum";
  if (s.format === "date-time") return "date-time";
  return Array.isArray(s.type) ? s.type.join(" | ") : (s.type ?? "any");
}

function typeLabel(s: Schema, spec: OpenApi): string {
  const { schema, nullable } = unwrapNull(s);
  const base = baseType(schema, spec);
  return nullable ? `${base} | null` : base;
}

function describe(s: Schema, spec: OpenApi): string | null {
  const { schema } = unwrapNull(s);
  return s.description ?? schema.description ?? resolve(schema, spec).description ?? null;
}

function fieldsOf(s: Schema, spec: OpenApi, prefix: string, depth: number): ResponseField[] {
  if (depth > MAX_DEPTH) return [];
  const object = objectOf(unwrapNull(s).schema, spec);
  if (!object?.properties) return [];
  return Object.entries(object.properties).flatMap(([name, sub]) => {
    const path = prefix ? `${prefix}.${name}` : name;
    const field: ResponseField = {
      path,
      type: typeLabel(sub, spec),
      required: object.required?.includes(name) ?? false,
      description: describe(sub, spec),
    };
    const inner = resolve(unwrapNull(sub).schema, spec);
    const nested =
      inner.type === "array" && inner.items
        ? fieldsOf(inner.items, spec, `${path}[]`, depth + 1)
        : fieldsOf(sub, spec, path, depth + 1);
    return [field, ...nested];
  });
}

function paramOf(p: Parameter, spec: OpenApi): EndpointParam {
  const schema = resolve(unwrapNull(p.schema).schema, spec);
  return {
    name: p.name,
    in: p.in,
    required: p.required ?? false,
    description: p.description ?? schema.description ?? null,
    type: schema.type === "integer" ? "integer" : "string",
    enum: schema.enum ?? null,
    min: schema.minimum ?? null,
    max: schema.maximum ?? null,
  };
}

const PARAM_ORDER = { path: 0, query: 1 } as const;

export function endpointsOf(spec: OpenApi): [Endpoint, ...Endpoint[]] {
  const [first, ...rest] = Object.entries(spec.paths).flatMap(([path, item]) =>
    Object.entries(item).map(([method, op]) => {
      const ok = op.responses["200"];
      const [contentType, media] = Object.entries(ok?.content ?? {})[0] ?? ["", undefined];
      const schema = media?.schema;
      return {
        id: op.operationId,
        method: method.toUpperCase(),
        path,
        family: familyOf(path),
        summary: op.summary ?? null,
        description: op.description ?? null,
        params: (op.parameters ?? [])
          .map((p) => paramOf(p, spec))
          .sort((a, b) => PARAM_ORDER[a.in] - PARAM_ORDER[b.in]),
        responseDescription: ok?.description ?? null,
        schemaName: schema?.$ref ? refName(schema.$ref) : schema ? typeLabel(schema, spec) : null,
        contentType,
        sse: contentType === SSE,
        fields: schema ? fieldsOf(schema, spec, "", 0) : [],
      };
    }),
  );
  if (!first) throw new Error("the contract has no operation");
  // ⚠️ A stable sort: within a family, the contract's order stays.
  return [first, ...rest].sort(
    (a, b) => FAMILIES.indexOf(a.family) - FAMILIES.indexOf(b.family),
  ) as [Endpoint, ...Endpoint[]];
}
