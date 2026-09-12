/**
 * Tiny parameter schema so every command declares its params once and the
 * registry stays introspectable. The CLI and MCP server will be generated
 * from these descriptors later; keep them data, not code.
 */

export type ParamSpec =
  | { type: 'string'; optional?: boolean; description?: string }
  | { type: 'number'; optional?: boolean; min?: number; max?: number; description?: string }
  | { type: 'boolean'; optional?: boolean; description?: string }
  | { type: 'enum'; values: readonly string[]; optional?: boolean; description?: string };

export type ParamSchema = Record<string, ParamSpec>;

type SpecToType<S extends ParamSpec> = S extends { type: 'string' }
  ? string
  : S extends { type: 'number' }
    ? number
    : S extends { type: 'boolean' }
      ? boolean
      : S extends { type: 'enum'; values: readonly (infer V)[] }
        ? V
        : never;

type OptionalKeys<S extends ParamSchema> = {
  [K in keyof S]: S[K] extends { optional: true } ? K : never;
}[keyof S];

type RequiredKeys<S extends ParamSchema> = Exclude<keyof S, OptionalKeys<S>>;

/** Infers the TypeScript params type from a schema literal. */
export type ParamsOf<S extends ParamSchema> = {
  [K in RequiredKeys<S>]: SpecToType<S[K]>;
} & {
  [K in OptionalKeys<S>]?: SpecToType<S[K]>;
};

export const p = {
  string: (description?: string) => ({ type: 'string', ...(description ? { description } : {}) }) as const,
  number: (opts: { min?: number; max?: number; description?: string } = {}) =>
    ({ type: 'number', ...opts }) as const,
  boolean: (description?: string) => ({ type: 'boolean', ...(description ? { description } : {}) }) as const,
  enum: <const V extends readonly string[]>(values: V, description?: string) =>
    ({ type: 'enum', values, ...(description ? { description } : {}) }) as const,
  optional: <const S extends ParamSpec>(spec: S) => ({ ...spec, optional: true }) as const,
};

export class CommandParamError extends Error {
  constructor(
    public readonly command: string,
    public readonly param: string,
    message: string,
  ) {
    super(`${command}: param "${param}" ${message}`);
    this.name = 'CommandParamError';
  }
}

export function validateParams(command: string, schema: ParamSchema, params: unknown): void {
  if (typeof params !== 'object' || params === null) {
    throw new CommandParamError(command, '*', 'params must be an object');
  }
  const obj = params as Record<string, unknown>;
  for (const [key, spec] of Object.entries(schema)) {
    const value = obj[key];
    if (value === undefined) {
      if (spec.optional) continue;
      throw new CommandParamError(command, key, 'is required');
    }
    switch (spec.type) {
      case 'string':
        if (typeof value !== 'string') throw new CommandParamError(command, key, 'must be a string');
        break;
      case 'boolean':
        if (typeof value !== 'boolean') throw new CommandParamError(command, key, 'must be a boolean');
        break;
      case 'number':
        if (typeof value !== 'number' || Number.isNaN(value)) {
          throw new CommandParamError(command, key, 'must be a number');
        }
        if (spec.min !== undefined && value < spec.min) {
          throw new CommandParamError(command, key, `must be >= ${spec.min}`);
        }
        if (spec.max !== undefined && value > spec.max) {
          throw new CommandParamError(command, key, `must be <= ${spec.max}`);
        }
        break;
      case 'enum':
        if (typeof value !== 'string' || !spec.values.includes(value)) {
          throw new CommandParamError(command, key, `must be one of ${spec.values.join(', ')}`);
        }
        break;
    }
  }
  for (const key of Object.keys(obj)) {
    if (!(key in schema)) throw new CommandParamError(command, key, 'is not a known param');
  }
}
