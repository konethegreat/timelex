// Written by Kone Tshivhinda & Claude
export * from "./client.js";
export * from "./types.js";

export type {
  components as GatewayComponents,
  operations as GatewayOperations,
  paths as GatewayPaths,
} from "./generated/gateway.js";
export type {
  components as IdentityComponents,
  operations as IdentityOperations,
  paths as IdentityPaths,
} from "./generated/identity.js";
export type {
  components as VaultComponents,
  operations as VaultOperations,
  paths as VaultPaths,
} from "./generated/vault.js";
