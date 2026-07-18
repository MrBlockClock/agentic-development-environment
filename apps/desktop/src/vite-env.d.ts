/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_ADE_API_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
