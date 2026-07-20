/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_ADE_API_URL?: string;
  readonly VITE_ADE_API_TOKEN?: string;
  readonly VITE_ADE_DEV_MODE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
